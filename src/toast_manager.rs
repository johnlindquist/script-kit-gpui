//! Toast Manager for coordinating toast notifications
//!
//! This module provides a `ToastManager` that handles:
//! - Notification queue with auto-dismiss timers
//! - Maximum visible toasts limit
//! - Toast positioning (top-right stack)
//! - Dismiss callbacks and lifecycle management
//!
//! # Integration with gpui-component
//!
//! The ToastManager acts as a staging queue. Toasts are pushed via `push()` from
//! anywhere in the code (even without window access). Then in the render loop,
//! call `drain_pending()` to get the pending toasts and push them to gpui-component's
//! notification system via `window.push_notification()`.
//!

#![allow(dead_code)]

use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::components::Toast;

/// A wrapper around Toast that tracks its lifecycle state
pub struct ToastNotification {
    /// Unique identifier for this toast
    pub id: String,
    /// The underlying toast component
    pub toast: Toast,
    /// When the toast was created
    pub created_at: Instant,
    /// Whether the toast has been dismissed
    pub is_dismissed: bool,
    /// Auto-dismiss duration (copied from toast for efficiency)
    duration_ms: Option<u64>,
}

impl std::fmt::Debug for ToastNotification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToastNotification")
            .field("id", &self.id)
            .field("created_at", &self.created_at)
            .field("is_dismissed", &self.is_dismissed)
            .field("duration_ms", &self.duration_ms)
            .finish_non_exhaustive()
    }
}

impl ToastNotification {
    /// Create a new toast notification wrapping a Toast
    pub fn new(toast: Toast) -> Self {
        let duration_ms = toast.get_duration_ms();
        Self {
            id: Uuid::new_v4().to_string(),
            toast,
            created_at: Instant::now(),
            is_dismissed: false,
            duration_ms,
        }
    }

    /// Create a toast notification with a specific ID (useful for testing)
    pub fn with_id(id: impl Into<String>, toast: Toast) -> Self {
        let duration_ms = toast.get_duration_ms();
        Self {
            id: id.into(),
            toast,
            created_at: Instant::now(),
            is_dismissed: false,
            duration_ms,
        }
    }

    /// Check if this toast should be auto-dismissed based on elapsed time
    pub fn should_auto_dismiss(&self) -> bool {
        if self.is_dismissed {
            return false;
        }

        if let Some(duration) = self.duration_ms {
            let elapsed = self.created_at.elapsed();
            elapsed >= Duration::from_millis(duration)
        } else {
            // Persistent toasts never auto-dismiss
            false
        }
    }

    /// Get the remaining time before auto-dismiss, if applicable
    pub fn remaining_ms(&self) -> Option<u64> {
        self.duration_ms.map(|duration| {
            let elapsed = self.created_at.elapsed().as_millis() as u64;
            duration.saturating_sub(elapsed)
        })
    }

    /// Mark this toast as dismissed
    pub fn dismiss(&mut self) {
        self.is_dismissed = true;
    }
}

/// Manager for handling toast notification queue and lifecycle
pub struct ToastManager {
    /// All active notifications (includes dismissed, waiting for cleanup)
    notifications: Vec<ToastNotification>,
    /// Maximum number of toasts visible at once
    max_visible: usize,
    /// Whether the manager needs a re-render
    needs_notify: bool,
}

impl std::fmt::Debug for ToastManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToastManager")
            .field("notification_count", &self.notifications.len())
            .field("max_visible", &self.max_visible)
            .field("needs_notify", &self.needs_notify)
            .finish()
    }
}

impl Default for ToastManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ToastManager {
    /// Create a new ToastManager with default settings
    pub fn new() -> Self {
        Self {
            notifications: Vec::new(),
            max_visible: 5,
            needs_notify: false,
        }
    }

    /// Create a ToastManager with a custom max_visible limit
    pub fn with_max_visible(max_visible: usize) -> Self {
        Self {
            notifications: Vec::new(),
            max_visible,
            needs_notify: false,
        }
    }

    /// Push a new toast onto the queue
    ///
    /// Returns the unique ID assigned to this toast, which can be used
    /// for later dismissal.
    pub fn push(&mut self, toast: Toast) -> String {
        let notification = ToastNotification::new(toast);
        let id = notification.id.clone();

        tracing::debug!(
            toast_id = %id,
            duration_ms = ?notification.duration_ms,
            "Toast pushed to queue"
        );

        self.notifications.push(notification);
        self.needs_notify = true;
        id
    }

    /// Push a toast with a specific ID (useful for testing or tracking)
    pub fn push_with_id(&mut self, id: impl Into<String>, toast: Toast) -> String {
        let notification = ToastNotification::with_id(id, toast);
        let id = notification.id.clone();

        tracing::debug!(
            toast_id = %id,
            duration_ms = ?notification.duration_ms,
            "Toast pushed to queue with custom ID"
        );

        self.notifications.push(notification);
        self.needs_notify = true;
        id
    }

    /// Dismiss a toast by its ID
    ///
    /// Returns true if the toast was found and dismissed, false otherwise.
    pub fn dismiss(&mut self, id: &str) -> bool {
        if let Some(notification) = self.notifications.iter_mut().find(|n| n.id == id) {
            if !notification.is_dismissed {
                notification.dismiss();
                self.needs_notify = true;

                tracing::debug!(
                    toast_id = %id,
                    "Toast dismissed"
                );

                return true;
            }
        }
        false
    }

    /// Dismiss all toasts
    pub fn dismiss_all(&mut self) {
        let count = self
            .notifications
            .iter()
            .filter(|n| !n.is_dismissed)
            .count();

        for notification in &mut self.notifications {
            notification.dismiss();
        }

        if count > 0 {
            self.needs_notify = true;
            tracing::debug!(count = count, "All toasts dismissed");
        }
    }

    /// Get the visible (non-dismissed) toasts, limited by max_visible
    ///
    /// Returns a slice of the most recent visible toasts.
    pub fn visible_toasts(&self) -> Vec<&ToastNotification> {
        self.notifications
            .iter()
            .filter(|n| !n.is_dismissed)
            .rev() // Most recent first for top-of-stack positioning
            .take(self.max_visible)
            .collect()
    }

    /// Get a mutable reference to all notifications (for rendering)
    pub fn notifications_mut(&mut self) -> &mut Vec<ToastNotification> {
        &mut self.notifications
    }

    /// Get count of visible (non-dismissed) toasts
    pub fn visible_count(&self) -> usize {
        self.notifications
            .iter()
            .filter(|n| !n.is_dismissed)
            .count()
    }

    /// Get total count of all toasts (including dismissed)
    pub fn total_count(&self) -> usize {
        self.notifications.len()
    }

    /// Check if there are any visible toasts
    pub fn has_visible(&self) -> bool {
        self.notifications.iter().any(|n| !n.is_dismissed)
    }

    /// Tick the manager to check for auto-dismiss timers
    ///
    /// This should be called periodically (e.g., in the render loop or via timer).
    /// Returns true if any toasts were auto-dismissed (triggering a re-render).
    pub fn tick(&mut self) -> bool {
        let mut dismissed_any = false;

        for notification in &mut self.notifications {
            if notification.should_auto_dismiss() {
                tracing::debug!(
                    toast_id = %notification.id,
                    "Toast auto-dismissed"
                );
                notification.dismiss();
                dismissed_any = true;
            }
        }

        if dismissed_any {
            self.needs_notify = true;
        }

        dismissed_any
    }

    /// Clean up dismissed toasts from the queue
    ///
    /// Call this periodically to free memory from dismissed toasts.
    /// Returns the number of toasts cleaned up.
    pub fn cleanup(&mut self) -> usize {
        let before_count = self.notifications.len();
        self.notifications.retain(|n| !n.is_dismissed);
        let cleaned = before_count - self.notifications.len();

        if cleaned > 0 {
            tracing::debug!(count = cleaned, "Cleaned up dismissed toasts");
        }

        cleaned
    }

    /// Check if the manager needs a UI notification (and reset the flag)
    ///
    /// Use this to determine if cx.notify() should be called.
    pub fn take_needs_notify(&mut self) -> bool {
        let needs = self.needs_notify;
        self.needs_notify = false;
        needs
    }

    /// Get the maximum visible toasts limit
    pub fn max_visible(&self) -> usize {
        self.max_visible
    }

    /// Set the maximum visible toasts limit
    pub fn set_max_visible(&mut self, max: usize) {
        self.max_visible = max;
    }

    /// Clear all toasts (both visible and dismissed)
    pub fn clear(&mut self) {
        if !self.notifications.is_empty() {
            self.needs_notify = true;
        }
        self.notifications.clear();

        tracing::debug!("Toast manager cleared");
    }

    /// Drain all pending (non-dismissed) toasts from the queue
    ///
    /// This is used to flush pending toasts to gpui-component's notification system.
    /// After draining, the internal queue is cleared.
    ///
    /// Returns an iterator of Toast references with their metadata.
    pub fn drain_pending(&mut self) -> Vec<PendingToast> {
        let pending: Vec<PendingToast> = self
            .notifications
            .drain(..)
            .filter(|n| !n.is_dismissed)
            .map(|n| PendingToast {
                message: n.toast.get_message().to_string(),
                variant: n.toast.get_variant(),
                details: n.toast.get_details().cloned(),
                duration_ms: n.toast.get_duration_ms(),
            })
            .collect();

        if !pending.is_empty() {
            tracing::debug!(count = pending.len(), "Drained pending toasts");
        }

        pending
    }
}

/// A pending toast ready to be converted to gpui-component Notification
#[derive(Debug, Clone)]
pub struct PendingToast {
    pub message: String,
    pub variant: crate::components::ToastVariant,
    pub details: Option<String>,
    pub duration_ms: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::ToastColors;

    fn make_test_toast(duration_ms: Option<u64>) -> Toast {
        let colors = ToastColors::default();
        Toast::new("Test message", colors).duration_ms(duration_ms)
    }

    #[test]
    fn test_toast_notification_creation() {
        let toast = make_test_toast(Some(5000));
        let notification = ToastNotification::new(toast);

        assert!(!notification.id.is_empty());
        assert!(!notification.is_dismissed);
        assert_eq!(notification.duration_ms, Some(5000));
    }

    #[test]
    fn test_toast_notification_with_id() {
        let toast = make_test_toast(Some(5000));
        let notification = ToastNotification::with_id("custom-id", toast);

        assert_eq!(notification.id, "custom-id");
    }

    #[test]
    fn test_toast_manager_push() {
        let mut manager = ToastManager::new();
        let toast = make_test_toast(Some(5000));

        let id = manager.push(toast);

        assert!(!id.is_empty());
        assert_eq!(manager.total_count(), 1);
        assert_eq!(manager.visible_count(), 1);
        assert!(manager.take_needs_notify());
    }

    #[test]
    fn test_toast_manager_dismiss() {
        let mut manager = ToastManager::new();
        let toast = make_test_toast(Some(5000));
        let id = manager.push(toast);

        // Reset notify flag
        manager.take_needs_notify();

        // Dismiss the toast
        let dismissed = manager.dismiss(&id);

        assert!(dismissed);
        assert_eq!(manager.visible_count(), 0);
        assert_eq!(manager.total_count(), 1); // Still in queue until cleanup
        assert!(manager.take_needs_notify());
    }

    #[test]
    fn test_toast_manager_dismiss_nonexistent() {
        let mut manager = ToastManager::new();
        let dismissed = manager.dismiss("nonexistent-id");

        assert!(!dismissed);
    }

    #[test]
    fn test_toast_manager_dismiss_all() {
        let mut manager = ToastManager::new();
        manager.push(make_test_toast(Some(5000)));
        manager.push(make_test_toast(Some(5000)));
        manager.push(make_test_toast(Some(5000)));

        manager.take_needs_notify();
        manager.dismiss_all();

        assert_eq!(manager.visible_count(), 0);
        assert_eq!(manager.total_count(), 3);
        assert!(manager.take_needs_notify());
    }

    #[test]
    fn test_toast_manager_visible_toasts_limit() {
        let mut manager = ToastManager::with_max_visible(2);

        manager.push(make_test_toast(Some(5000)));
        manager.push(make_test_toast(Some(5000)));
        manager.push(make_test_toast(Some(5000)));

        let visible = manager.visible_toasts();

        assert_eq!(visible.len(), 2);
        assert_eq!(manager.total_count(), 3);
    }

    #[test]
    fn test_toast_manager_cleanup() {
        let mut manager = ToastManager::new();
        let id1 = manager.push(make_test_toast(Some(5000)));
        manager.push(make_test_toast(Some(5000)));

        manager.dismiss(&id1);
        let cleaned = manager.cleanup();

        assert_eq!(cleaned, 1);
        assert_eq!(manager.total_count(), 1);
    }

    #[test]
    fn test_toast_manager_clear() {
        let mut manager = ToastManager::new();
        manager.push(make_test_toast(Some(5000)));
        manager.push(make_test_toast(Some(5000)));

        manager.take_needs_notify();
        manager.clear();

        assert_eq!(manager.total_count(), 0);
        assert!(manager.take_needs_notify());
    }

    #[test]
    fn test_toast_notification_should_auto_dismiss() {
        // Create a toast with very short duration
        let colors = ToastColors::default();
        let toast = Toast::new("Test", colors).duration_ms(Some(1));
        let notification = ToastNotification::new(toast);

        // Wait a bit for the duration to pass
        std::thread::sleep(std::time::Duration::from_millis(5));

        assert!(notification.should_auto_dismiss());
    }

    #[test]
    fn test_toast_notification_persistent() {
        let colors = ToastColors::default();
        let toast = Toast::new("Test", colors).duration_ms(None);
        let notification = ToastNotification::new(toast);

        // Persistent toasts should never auto-dismiss
        assert!(!notification.should_auto_dismiss());
    }

    #[test]
    fn test_toast_manager_tick() {
        let mut manager = ToastManager::new();

        // Add a toast with very short duration
        let colors = ToastColors::default();
        let toast = Toast::new("Test", colors).duration_ms(Some(1));
        manager.push(toast);

        // Wait for duration to pass
        std::thread::sleep(std::time::Duration::from_millis(5));

        // Tick should auto-dismiss
        let dismissed = manager.tick();

        assert!(dismissed);
        assert_eq!(manager.visible_count(), 0);
    }

    #[test]
    fn test_toast_notification_remaining_ms() {
        let colors = ToastColors::default();
        let toast = Toast::new("Test", colors).duration_ms(Some(5000));
        let notification = ToastNotification::new(toast);

        let remaining = notification.remaining_ms();

        assert!(remaining.is_some());
        assert!(remaining.unwrap() <= 5000);
        assert!(remaining.unwrap() > 4900); // Should be close to 5000
    }

    #[test]
    fn test_toast_manager_has_visible() {
        let mut manager = ToastManager::new();

        assert!(!manager.has_visible());

        let id = manager.push(make_test_toast(Some(5000)));
        assert!(manager.has_visible());

        manager.dismiss(&id);
        assert!(!manager.has_visible());
    }
}
