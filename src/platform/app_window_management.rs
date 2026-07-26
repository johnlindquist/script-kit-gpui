use crate::logging;
use crate::windows::DisplayBounds;

#[cfg(target_os = "macos")]
use cocoa::appkit::NSApp;
#[cfg(target_os = "macos")]
use cocoa::base::{id, nil};
#[cfg(target_os = "macos")]
use cocoa::foundation::NSString as CocoaNSString;
#[cfg(target_os = "macos")]
use objc::{class, msg_send, sel, sel_impl};

#[cfg(target_os = "macos")]
use crate::window_manager;

/// NSWindowCollectionBehaviorCanJoinAllSpaces constant value (1 << 0 = 1)
#[cfg(target_os = "macos")]
const NS_WINDOW_COLLECTION_BEHAVIOR_CAN_JOIN_ALL_SPACES: u64 = 1 << 0;
#[cfg(target_os = "macos")]
const NS_APPLICATION_ACTIVATION_POLICY_REGULAR: i64 = 0;
#[cfg(target_os = "macos")]
const NS_APPLICATION_ACTIVATION_POLICY_ACCESSORY: i64 = 1;
#[cfg(target_os = "macos")]
const NS_WINDOW_ANIMATION_BEHAVIOR_NONE: i64 = 2;

#[cfg(target_os = "macos")]
pub(crate) const fn main_panel_collection_behavior(current: u64) -> u64 {
    (current & !NS_WINDOW_COLLECTION_BEHAVIOR_CAN_JOIN_ALL_SPACES)
        | NS_WINDOW_COLLECTION_BEHAVIOR_MOVE_TO_ACTIVE_SPACE
        | NS_WINDOW_COLLECTION_BEHAVIOR_FULL_SCREEN_AUXILIARY
        | NS_WINDOW_COLLECTION_BEHAVIOR_IGNORES_CYCLE
}

// ============================================================================
// Thread Safety
// ============================================================================

/// Check whether the current thread is the main thread (works in release builds).
#[cfg(target_os = "macos")]
fn is_main_thread() -> bool {
    // SAFETY: NSThread.isMainThread is a class method that only reads thread
    // identity. It is safe to call from any thread and does not mutate state.
    unsafe {
        let is_main: bool = msg_send![class!(NSThread), isMainThread];
        is_main
    }
}

/// Runtime guard: logs an error and returns `true` (caller should bail)
/// when called from a non-main thread. Works in both debug and release builds.
#[cfg(target_os = "macos")]
pub(crate) fn require_main_thread(fn_name: &str) -> bool {
    if !is_main_thread() {
        logging::log(
            "ERROR",
            &format!(
                "{} called from non-main thread; AppKit requires main thread",
                fn_name
            ),
        );
        return true;
    }
    false
}

// ============================================================================
// Application Activation Policy
// ============================================================================

/// Configure the app as an "accessory" application.
///
/// This is equivalent to setting `LSUIElement=true` in Info.plist, but done at runtime.
/// Accessory apps:
/// - Do NOT appear in the Dock
/// - Do NOT take menu bar ownership when activated
/// - Can still show windows that float above other apps
///
/// This is critical for window management actions (tile, maximize, etc.) because
/// it allows us to query `menuBarOwningApplication` to find the previously active app.
///
/// # macOS Behavior
///
/// Sets NSApplicationActivationPolicyAccessory (value = 1) on the app.
/// Must be called early in app startup, before any windows are shown.
///
/// # Other Platforms
///
/// No-op on non-macOS platforms.
#[cfg(target_os = "macos")]
pub fn configure_as_accessory_app() {
    if require_main_thread("configure_as_accessory_app") {
        return;
    }
    // SAFETY: Main thread verified. NSApp() is always valid after app launch.
    unsafe {
        let app: id = NSApp();
        // NSApplicationActivationPolicyAccessory = 1
        // This makes the app not appear in Dock and not take menu bar ownership
        let _: () = msg_send![
            app,
            setActivationPolicy: NS_APPLICATION_ACTIVATION_POLICY_ACCESSORY
        ];
        logging::log(
            "PANEL",
            "Configured app as accessory (no Dock icon, no menu bar ownership)",
        );
    }
}

#[cfg(not(target_os = "macos"))]
pub fn configure_as_accessory_app() {
    // No-op on non-macOS platforms
}

/// Temporarily switch to "regular" app mode so the app appears in Cmd+Tab.
///
/// This is used when the AI window is opened - it needs to be Cmd+Tab accessible
/// unlike the main menu which is a utility panel.
///
/// # macOS Behavior
///
/// Sets NSApplicationActivationPolicyRegular (value = 0) on the app.
/// The app will appear in the Dock and Cmd+Tab while in this mode.
///
/// # Other Platforms
///
/// No-op on non-macOS platforms.
#[cfg(target_os = "macos")]
pub fn set_regular_app_mode() {
    if require_main_thread("set_regular_app_mode") {
        return;
    }
    // SAFETY: Main thread verified. NSApp() is always valid after app launch.
    unsafe {
        let app: id = NSApp();
        // NSApplicationActivationPolicyRegular = 0
        // This makes the app appear in Dock and Cmd+Tab
        let _: () = msg_send![app, setActivationPolicy: NS_APPLICATION_ACTIVATION_POLICY_REGULAR];
        logging::log(
            "PANEL",
            "Switched to regular app mode (appears in Dock and Cmd+Tab)",
        );
    }
}

#[cfg(not(target_os = "macos"))]
pub fn set_regular_app_mode() {
    // No-op on non-macOS platforms
}

/// Switch back to "accessory" app mode after AI window is closed.
///
/// This restores the app to its normal state where it doesn't appear in
/// the Dock or Cmd+Tab.
///
/// # macOS Behavior
///
/// Sets NSApplicationActivationPolicyAccessory (value = 1) on the app.
///
/// # Other Platforms
///
/// No-op on non-macOS platforms.
#[cfg(target_os = "macos")]
pub fn set_accessory_app_mode() {
    if require_main_thread("set_accessory_app_mode") {
        return;
    }
    // SAFETY: Main thread verified. NSApp() is always valid after app launch.
    unsafe {
        let app: id = NSApp();
        // NSApplicationActivationPolicyAccessory = 1
        let _: () = msg_send![
            app,
            setActivationPolicy: NS_APPLICATION_ACTIVATION_POLICY_ACCESSORY
        ];
        logging::log(
            "PANEL",
            "Switched to accessory app mode (no Dock icon, no Cmd+Tab)",
        );
    }
}

#[cfg(not(target_os = "macos"))]
pub fn set_accessory_app_mode() {
    // No-op on non-macOS platforms
}

/// Send the AI window to the back (behind other apps' windows).
///
/// This is called when the main menu is shown to prevent the AI window
/// from being brought forward along with the main menu. The AI window
/// should only come forward via Cmd+Tab or explicit user action.
///
/// # macOS Behavior
///
/// Finds the AI window by title and uses orderBack: to send it behind
/// other windows without hiding it.
///
/// # Other Platforms
///
/// No-op on non-macOS platforms.
#[cfg(target_os = "macos")]
pub fn send_ai_window_to_back() {
    if require_main_thread("send_ai_window_to_back") {
        return;
    }
    // SAFETY: Main thread verified. NSApp() is always valid. We check title
    // and UTF8String for nil/null before dereferencing via CStr::from_ptr.
    unsafe {
        use std::ffi::CStr;

        let app: id = NSApp();
        let windows: id = msg_send![app, windows];
        if windows.is_null() {
            return;
        }
        let count: usize = msg_send![windows, count];

        for i in 0..count {
            let window: id = msg_send![windows, objectAtIndex: i];
            let title: id = msg_send![window, title];

            if title != nil {
                let title_cstr: *const i8 = msg_send![title, UTF8String];
                if !title_cstr.is_null() {
                    let title_str = CStr::from_ptr(title_cstr).to_string_lossy();

                    if title_str == "Script Kit Agent Chat" {
                        // Found the AI window - send it to the back
                        let _: () = msg_send![window, orderBack: nil];
                        logging::log(
                            "PANEL",
                            "AI window sent to back (won't come forward with main menu)",
                        );
                        return;
                    }
                }
            }
        }
        // AI window not found - that's fine, it may not be open
    }
}

#[cfg(not(target_os = "macos"))]
pub fn send_ai_window_to_back() {
    // No-op on non-macOS platforms
}

// ============================================================================
// Space Management
// ============================================================================

/// Ensure the main window moves to the currently active macOS space when shown.
///
/// This function sets NSWindowCollectionBehaviorMoveToActiveSpace on the main window,
/// which causes it to move to whichever space is currently active when the window
/// becomes visible, rather than forcing the user back to the space where the window
/// was last shown.
///
/// # macOS Behavior
///
/// Uses the WindowManager to get the main window (not keyWindow, which may not exist
/// yet during app startup) and sets the collection behavior.
///
/// # Other Platforms
///
/// No-op on non-macOS platforms.
///
/// # Safety
///
/// Uses Objective-C message sending internally on macOS.
#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub fn ensure_move_to_active_space() {
    if require_main_thread("ensure_move_to_active_space") {
        return;
    }
    // SAFETY: Main thread verified. Window pointer from WindowManager is valid.
    // collectionBehavior / setCollectionBehavior are standard NSWindow methods.
    unsafe {
        // Use WindowManager to get the main window (not keyWindow, which may not exist yet)
        let window = match window_manager::get_main_window() {
            Some(w) => w,
            None => {
                logging::log(
                    "PANEL",
                    "WARNING: Main window not registered, cannot set collection behavior",
                );
                return;
            }
        };

        // Get current collection behavior to preserve existing flags
        let current: u64 = msg_send![window, collectionBehavior];

        // Strip CanJoinAllSpaces before adding MoveToActiveSpace; those
        // flags are mutually exclusive and all-spaces leaks across Spaces.
        let desired = main_panel_collection_behavior(current);

        let _: () = msg_send![window, setCollectionBehavior:desired];

        logging::log(
            "PANEL",
            &format!(
                "Set collection behavior: {} -> {} (CanJoinAllSpaces=false, MoveToActiveSpace=true, FullScreenAuxiliary=true)",
                current, desired
            ),
        );
    }
}

#[cfg(not(target_os = "macos"))]
pub fn ensure_move_to_active_space() {
    // No-op on non-macOS platforms
}

// ============================================================================
// Floating Panel Configuration
// ============================================================================

/// Configure the main window as a floating macOS panel.
///
/// This function configures the main window (via WindowManager) with:
/// - Preserved GPUI-assigned window level for PopUp windows
/// - MoveToActiveSpace collection behavior when compatible with existing flags
/// - Disabled window restoration - prevents macOS from remembering window position
/// - Empty frame autosave name - prevents position caching
///
/// # macOS Behavior
///
/// Uses WindowManager to get the main window (more reliable than NSApp.keyWindow,
/// which is timing-sensitive and can return nil during startup or the wrong window
/// in multi-window scenarios). If no main window is registered, logs a warning.
///
/// # Other Platforms
///
/// No-op on non-macOS platforms.
///
/// # Safety
///
/// Uses Objective-C message sending internally on macOS.
///
#[cfg(target_os = "macos")]
pub fn configure_as_floating_panel() {
    if require_main_thread("configure_as_floating_panel") {
        return;
    }
    // SAFETY: Main thread verified. Window from WindowManager is valid.
    // All msg_send! calls target standard NSWindow property setters.
    unsafe {
        // Use WindowManager to get the main window (more reliable than keyWindow)
        // keyWindow is timing-sensitive and can return nil during startup,
        // or the wrong window (Notes/AI) in multi-window scenarios.
        let window = match window_manager::get_main_window() {
            Some(w) => w,
            None => {
                logging::log(
                    "PANEL",
                    "WARNING: Main window not registered, cannot configure as floating panel",
                );
                return;
            }
        };

        // Do not override level - GPUI owns the native PopUp window level.

        // Get current collection behavior to preserve existing flags set by GPUI/AppKit
        let current: u64 = msg_send![window, collectionBehavior];

        // OR in our desired flags after stripping CanJoinAllSpaces:
        // - FullScreenAuxiliary: window can show over fullscreen apps without disrupting
        // - IgnoresCycle: exclude from Cmd+Tab app switcher (main window is a utility)
        // - MoveToActiveSpace: summon the launcher into the active Space when shown
        let desired = main_panel_collection_behavior(current);

        let _: () = msg_send![window, setCollectionBehavior:desired];

        // CRITICAL: Disable macOS window state restoration
        // This prevents macOS from remembering and restoring the window position
        // when the app is relaunched or the window is shown again
        let _: () = msg_send![window, setRestorable:false];

        // Also disable the window's autosave frame name which can cause position caching
        let empty_string: id = msg_send![class!(NSString), string];
        let _: () = msg_send![window, setFrameAutosaveName:empty_string];

        // Disable close/hide animation for instant dismiss (NSWindowAnimationBehaviorNone = 2)
        let _: () = msg_send![window, setAnimationBehavior: NS_WINDOW_ANIMATION_BEHAVIOR_NONE];

        // Log detailed breakdown of collection behavior bits
        let has_can_join = (desired & NS_WINDOW_COLLECTION_BEHAVIOR_CAN_JOIN_ALL_SPACES) != 0;
        let has_ignores = (desired & NS_WINDOW_COLLECTION_BEHAVIOR_IGNORES_CYCLE) != 0;
        let has_move_to_active =
            (desired & NS_WINDOW_COLLECTION_BEHAVIOR_MOVE_TO_ACTIVE_SPACE) != 0;

        logging::log(
            "PANEL",
            &format!(
                "Main window: behavior={}->{} [CanJoinAllSpaces={}, IgnoresCycle={}, MoveToActiveSpace={}]",
                current, desired, has_can_join, has_ignores, has_move_to_active
            ),
        );
        logging::log(
            "PANEL",
            "Main window: Will NOT appear in Cmd+Tab app switcher (floating utility panel)",
        );

        // Install cursor rect management so the underlying app's cursor
        // (e.g. Terminal's I-beam) doesn't bleed through our panel.
        install_cursor_tracking();
    }
}

#[cfg(not(target_os = "macos"))]
pub fn configure_as_floating_panel() {
    // No-op on non-macOS platforms
}

/// Idempotent main-panel setup that only records success after the
/// post-configure invariant report passes.
///
/// Oracle-Session `window-activation-invariants-guard` PR1 — the `PANEL_CONFIGURED`
/// atomic was previously set unconditionally after `configure_as_floating_panel()`
/// returned, but that function returns `()` and early-returns when the main
/// window is not registered yet. The atomic could flip to `true` without any
/// configuration actually happening, defeating the one-shot guard.
///
/// This helper replaces the six duplicated
/// `PANEL_CONFIGURED.load`/`configure_*`/`PANEL_CONFIGURED.store` blocks across
/// `src/main_sections/window_visibility.rs`, `src/main_entry/app_run_setup.rs`,
/// `src/main_entry/runtime_stdin_match_core.rs`, and
/// `src/main_entry/runtime_stdin.rs`. The atomic is stored only when the
/// post-configure invariant report is clean.
///
/// Returns `true` when the panel is (now or already was) configured and
/// passes invariants. Returns `false` when configuration did not converge
/// — callers can still proceed to show the window, but the one-shot guard
/// stays open so the next show path retries.
pub fn ensure_main_panel_configured(context: &'static str) -> bool {
    // The PANEL_CONFIGURED static lives at the crate root (see
    // `src/main.rs:279`). It is a caller-owned one-shot that must only flip
    // to `true` after a successful post-configure invariant report.
    #[cfg(target_os = "macos")]
    install_main_window_space_change_hide_observer();

    if crate::PANEL_CONFIGURED.load(std::sync::atomic::Ordering::SeqCst) {
        return true;
    }

    // The boot-time accessory transform can silently fail for unbundled
    // binaries: macOS may return NO from setActivationPolicy before NSApp
    // finishes launching, leaving the process at Prohibited(2). Observed
    // 2026-07-26 in every driver-launched probe session: "Configured app as
    // accessory" logged at boot, yet [NSApp activationPolicy] read 2 121ms
    // later, panicking the AfterConfigure invariant. ensure_* re-applies the
    // transform here — by ensure-time the app is running, where the
    // transform reliably takes — so the invariant asserts a converged state
    // instead of a boot race.
    #[cfg(target_os = "macos")]
    unsafe {
        let app: id = NSApp();
        let policy: i64 = msg_send![app, activationPolicy];
        if policy != NS_APPLICATION_ACTIVATION_POLICY_ACCESSORY {
            logging::log(
                "PANEL",
                &format!(
                    "ensure_main_panel_configured({}): activationPolicy={} — re-applying accessory transform",
                    context, policy
                ),
            );
            configure_as_accessory_app();
        }
    }

    configure_as_floating_panel();
    swizzle_gpui_blurred_view();

    let theme = crate::theme::get_cached_theme();
    let is_dark = theme.should_use_dark_vibrancy();
    let material = theme.get_vibrancy().material;
    configure_window_vibrancy_material_for_appearance(is_dark, material);

    #[cfg(target_os = "macos")]
    {
        let report = assert_main_panel_invariants(
            context,
            PanelInvariantPhase::AfterConfigure,
        );
        if report.ok() {
            crate::PANEL_CONFIGURED.store(true, std::sync::atomic::Ordering::SeqCst);
            true
        } else {
            logging::log(
                "PANEL",
                &format!(
                    "ensure_main_panel_configured({}): invariants failed — leaving one-shot open for retry",
                    context
                ),
            );
            false
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = context;
        crate::PANEL_CONFIGURED.store(true, std::sync::atomic::Ordering::SeqCst);
        true
    }
}
