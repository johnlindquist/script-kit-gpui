//! Clipboard change detection
//!
//! Platform-specific efficient clipboard change detection.
//! Uses NSPasteboard changeCount on macOS for cheap polling (no payload reads).

#[cfg(target_os = "macos")]
use objc::sel;
#[cfg(target_os = "macos")]
use objc::sel_impl;

use tracing::debug;

/// Get the current clipboard change count.
///
/// On macOS, this reads NSPasteboard.generalPasteboard.changeCount which is
/// a cheap integer read. Returns None on other platforms.
///
/// The change count is an integer that increments each time the clipboard
/// content changes. By comparing consecutive values, we can detect changes
/// without reading the actual clipboard payload.
#[cfg(target_os = "macos")]
pub fn get_pasteboard_change_count() -> Option<i64> {
    if let Err(error) =
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemClipboard)
    {
        tracing::warn!(%error, "Clipboard change-count access refused");
        return None;
    }
    use cocoa::appkit::NSPasteboard;
    use cocoa::base::nil;
    use objc::runtime::Object;

    // SAFETY: NSPasteboard.generalPasteboard is a class method returning the shared
    // pasteboard singleton. The pointer is nil-checked. changeCount returns a value type.
    unsafe {
        let pasteboard: *mut Object = NSPasteboard::generalPasteboard(nil);
        if pasteboard.is_null() {
            return None;
        }

        // changeCount is an NSInteger (i64 on 64-bit)
        let change_count: i64 = objc::msg_send![pasteboard, changeCount];
        Some(change_count)
    }
}

/// Fallback for non-macOS platforms: always returns None (use content-based detection)
#[cfg(not(target_os = "macos"))]
pub fn get_pasteboard_change_count() -> Option<i64> {
    None
}

/// Clipboard change detector that uses OS-level change counts when available.
///
/// On macOS, uses NSPasteboard changeCount for efficient polling.
/// On other platforms, falls back to None and caller should use content-based detection.
#[derive(Debug, Default)]
pub struct ClipboardChangeDetector {
    last_change_count: Option<i64>,
}

impl ClipboardChangeDetector {
    pub fn new() -> Self {
        Self {
            last_change_count: None,
        }
    }

    /// Check if the clipboard has changed since the last check.
    ///
    /// Returns:
    /// - `Some(true)` if clipboard changed (change count incremented)
    /// - `Some(false)` if clipboard unchanged (same change count)
    /// - `None` if change detection unavailable (caller should use content-based detection)
    ///
    /// The first call will always return `Some(true)` to trigger initial capture.
    pub fn has_changed(&mut self) -> Option<bool> {
        let current = get_pasteboard_change_count()?;

        let changed = match self.last_change_count {
            Some(last) => current != last,
            None => true, // First check, treat as changed
        };

        if changed {
            debug!(
                old_count = self.last_change_count,
                new_count = current,
                "Clipboard change detected via changeCount"
            );
        }

        self.last_change_count = Some(current);
        Some(changed)
    }

    /// Reset the detector state.
    ///
    /// Next call to `has_changed()` will return true.
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.last_change_count = None;
    }

    /// Get the last known change count (for debugging).
    #[cfg(test)]
    pub fn last_count(&self) -> Option<i64> {
        self.last_change_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_change_detector_first_call_returns_changed() {
        let mut detector = ClipboardChangeDetector::new();

        // On macOS, first call should return Some(true)
        // On other platforms, returns None
        #[cfg(target_os = "macos")]
        {
            let result = detector.has_changed();
            assert_eq!(result, Some(true), "First call should detect 'change'");
            assert!(
                detector.last_count().is_some(),
                "Should have stored change count"
            );
        }

        #[cfg(not(target_os = "macos"))]
        {
            let result = detector.has_changed();
            assert_eq!(result, None, "Non-macOS should return None");
        }
    }

    #[test]
    fn test_change_detector_consecutive_calls_without_change() {
        let mut detector = ClipboardChangeDetector::new();

        #[cfg(target_os = "macos")]
        {
            // First call initializes the detector
            let _ = detector.has_changed();

            // Second call should return Some(bool) — false if clipboard unchanged,
            // true if another process/test mutated the clipboard in between.
            // We can only assert the return type is Some, not the exact value,
            // because parallel tests or system processes may change the clipboard.
            let result = detector.has_changed();
            assert!(
                result.is_some(),
                "Consecutive call should return Some on macOS"
            );
        }
    }

    #[test]
    fn test_reset_causes_next_check_to_return_changed() {
        let mut detector = ClipboardChangeDetector::new();

        #[cfg(target_os = "macos")]
        {
            // Initialize
            let _ = detector.has_changed();
            let _ = detector.has_changed();

            // Reset
            detector.reset();
            assert!(
                detector.last_count().is_none(),
                "Reset should clear last count"
            );

            // Next call should return changed
            let result = detector.has_changed();
            assert_eq!(result, Some(true), "Post-reset call should return changed");
        }
    }

    #[test]
    fn test_get_pasteboard_change_count_returns_valid_value() {
        #[cfg(target_os = "macos")]
        {
            let count = get_pasteboard_change_count();
            assert!(count.is_some(), "Should get change count on macOS");
            // Change count is typically a positive integer
            assert!(
                count.unwrap() >= 0,
                "Change count should be non-negative: {:?}",
                count
            );
        }

        #[cfg(not(target_os = "macos"))]
        {
            let count = get_pasteboard_change_count();
            assert!(count.is_none(), "Should return None on non-macOS");
        }
    }
}
