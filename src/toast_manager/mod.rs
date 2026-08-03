//! Toast Manager for coordinating toast notifications.
//!
//! This module provides a `ToastManager` staging queue. Toasts are pushed via
//! `push()` from anywhere in the code (even without window access). In the
//! render loop, call `drain_pending()` to convert queued toasts into
//! gpui-component notifications.

#[allow(dead_code)]
pub(crate) mod notification;

use crate::components::{Toast, ToastVariant};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// A wrapper around Toast with a generated identifier.
pub struct ToastNotification {
    /// Unique identifier for this toast.
    pub id: String,
    /// The underlying toast component.
    pub toast: Toast,
    /// Number of coalesced occurrences represented by this toast.
    pub repeats: u32,
}

impl std::fmt::Debug for ToastNotification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToastNotification")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl ToastNotification {
    /// Create a new toast notification wrapping a Toast.
    pub fn new(toast: Toast) -> Self {
        Self {
            id: toast.get_id().as_str().to_string(),
            toast,
            repeats: 1,
        }
    }
}

#[derive(Debug, Clone)]
struct RecentToast {
    id: String,
    count: u32,
    first_seen: Instant,
}

/// Manager for handling the toast queue.
pub struct ToastManager {
    notifications: Vec<ToastNotification>,
    recent: HashMap<String, RecentToast>,
    coalesce_window: Duration,
}

impl std::fmt::Debug for ToastManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToastManager")
            .field("notification_count", &self.notifications.len())
            .field("recent_count", &self.recent.len())
            .field("coalesce_window_ms", &self.coalesce_window.as_millis())
            .finish()
    }
}

impl Default for ToastManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ToastManager {
    /// Create a new ToastManager with default settings.
    pub fn new() -> Self {
        Self {
            notifications: Vec::new(),
            recent: HashMap::new(),
            coalesce_window: Duration::from_millis(250),
        }
    }

    fn toast_key(toast: &Toast) -> String {
        format!("{:?}|{}", toast.get_variant(), toast.get_message())
    }

    /// Push a new toast onto the queue.
    ///
    /// Returns the unique ID assigned to this toast.
    pub fn push(&mut self, toast: Toast) -> String {
        let now = Instant::now();
        let key = Self::toast_key(&toast);
        let queue_len_before = self.notifications.len();

        if let Some((existing_id, existing_count, first_seen)) = self
            .recent
            .get(&key)
            .map(|recent| (recent.id.clone(), recent.count, recent.first_seen))
        {
            let elapsed = now.saturating_duration_since(first_seen);
            if elapsed <= self.coalesce_window {
                let updated_count = existing_count.saturating_add(1).min(999);

                if let Some(notification) = self
                    .notifications
                    .iter_mut()
                    .find(|notification| notification.id == existing_id)
                {
                    notification.repeats = updated_count;

                    if let Some(recent) = self.recent.get_mut(&key) {
                        recent.count = updated_count;
                    }

                    tracing::debug!(
                        event = "toast_manager_push_coalesced",
                        toast_id = %existing_id,
                        toast_key = %key,
                        repeats = updated_count,
                        queue_len = queue_len_before,
                        "Coalesced duplicate toast within window"
                    );

                    return existing_id;
                }

                tracing::warn!(
                    event = "toast_manager_recent_missing_notification",
                    toast_id = %existing_id,
                    toast_key = %key,
                    elapsed_ms = elapsed.as_millis(),
                    "Recent toast entry referenced missing notification; removing stale entry"
                );
                self.recent.remove(&key);
            }
        }

        let notification = ToastNotification::new(toast);
        let id = notification.id.clone();

        tracing::debug!(
            event = "toast_manager_push_enqueued",
            toast_id = %id,
            toast_key = %key,
            queue_len_before = queue_len_before,
            "Toast enqueued"
        );

        self.notifications.push(notification);
        self.recent.insert(
            key,
            RecentToast {
                id: id.clone(),
                count: 1,
                first_seen: now,
            },
        );

        id
    }

    /// Drain all pending toasts from the queue.
    ///
    /// This is used to flush queued toasts to gpui-component's notification
    /// system. After draining, the internal queue is cleared.
    pub fn drain_pending(&mut self) -> Vec<PendingToast> {
        let pending: Vec<PendingToast> = self
            .notifications
            .drain(..)
            .map(|notification| {
                let mut message = notification.toast.get_message().to_string();
                if notification.repeats > 1 {
                    message.push_str(&format!(" (x{})", notification.repeats));
                }

                // Runtime notifications currently preserve only simple message
                // feedback. Any Some(duration) uses gpui-component's default
                // autohide duration; None maps to a persistent notification.
                let toast = notification.toast.message(message.clone());
                PendingToast {
                    message,
                    variant: toast.get_variant(),
                    duration_ms: toast.get_duration_ms(),
                    persistent: toast.get_duration_ms().is_none(),
                    toast,
                }
            })
            .collect();

        if !pending.is_empty() {
            self.recent.clear();
            tracing::debug!(
                event = "toast_manager_drain_pending",
                count = pending.len(),
                "Drained pending toasts"
            );
        }

        pending
    }
}

/// A pending toast ready to be converted to a gpui-component Notification.
#[derive(Clone)]
pub struct PendingToast {
    pub message: String,
    pub variant: ToastVariant,
    pub duration_ms: Option<u64>,
    pub persistent: bool,
    pub toast: Toast,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{ToastAction, ToastColors};
    use std::thread;

    fn make_test_toast(message: &'static str, duration_ms: Option<u64>) -> Toast {
        make_test_toast_with_variant(message, duration_ms, ToastVariant::Info)
    }

    fn make_test_toast_with_variant(
        message: &'static str,
        duration_ms: Option<u64>,
        variant: ToastVariant,
    ) -> Toast {
        let colors = ToastColors::default();
        Toast::new(message, colors)
            .duration_ms(duration_ms)
            .variant(variant)
    }

    #[test]
    fn test_push_returns_generated_toast_id() {
        let mut manager = ToastManager::new();

        let id = manager.push(make_test_toast("Test message", Some(5000)));

        assert!(!id.is_empty());
    }

    #[test]
    fn test_drain_pending_returns_enqueued_toasts_in_fifo_order() {
        let mut manager = ToastManager::new();

        manager.push(make_test_toast("First", Some(4000)));
        manager.push(make_test_toast("Second", None));

        let pending = manager.drain_pending();
        assert_eq!(pending.len(), 2);

        assert_eq!(pending[0].message, "First");
        assert_eq!(pending[0].variant, ToastVariant::Info);
        assert!(!pending[0].persistent);

        assert_eq!(pending[1].message, "Second");
        assert_eq!(pending[1].variant, ToastVariant::Info);
        assert!(pending[1].persistent);
    }

    #[test]
    fn drain_preserves_identity_actions_dismissibility_and_exact_duration() {
        let mut manager = ToastManager::new();
        let toast = Toast::new("Actionable", ToastColors::default())
            .with_id("fixture:actionable")
            .duration_ms(Some(1_234))
            .action(ToastAction::new("retry", "Retry", Box::new(|_, _, _| {})));

        manager.push(toast);
        let pending = manager.drain_pending();

        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].toast.get_id().as_str(), "fixture:actionable");
        assert_eq!(pending[0].toast.get_actions().len(), 1);
        assert_eq!(pending[0].toast.get_actions()[0].id.as_str(), "retry");
        assert!(pending[0].toast.is_dismissible());
        assert_eq!(pending[0].duration_ms, Some(1_234));
    }

    #[test]
    fn test_drain_pending_collapses_some_duration_to_non_persistent_contract() {
        let mut manager = ToastManager::new();

        manager.push(make_test_toast("Short", Some(1000)));
        manager.push(make_test_toast("Long", Some(10_000)));
        manager.push(make_test_toast("Pinned", None));

        let pending = manager.drain_pending();
        assert_eq!(pending.len(), 3);
        assert!(
            !pending[0].persistent,
            "Some(duration) uses gpui-component's default autohide"
        );
        assert!(
            !pending[1].persistent,
            "custom Some(duration) values are not runtime durations"
        );
        assert!(
            pending[2].persistent,
            "None is the only persistent notification marker"
        );
    }

    #[test]
    fn test_drain_pending_clears_queue_after_first_drain() {
        let mut manager = ToastManager::new();
        manager.push(make_test_toast("Only toast", Some(2000)));

        let first_drain = manager.drain_pending();
        let second_drain = manager.drain_pending();

        assert_eq!(first_drain.len(), 1);
        assert!(second_drain.is_empty());
    }

    #[test]
    fn test_push_coalesces_duplicate_toasts_within_window() {
        let mut manager = ToastManager::new();

        let first_id = manager.push(make_test_toast("Duplicate", Some(3000)));
        let second_id = manager.push(make_test_toast("Duplicate", Some(3000)));

        assert_eq!(first_id, second_id);

        let pending = manager.drain_pending();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].message, "Duplicate (x2)");
    }

    #[test]
    fn test_push_uses_variant_and_message_for_coalesce_key() {
        let mut manager = ToastManager::new();

        let info_id = manager.push(make_test_toast_with_variant(
            "Same message",
            Some(3000),
            ToastVariant::Info,
        ));
        let error_id = manager.push(make_test_toast_with_variant(
            "Same message",
            Some(3000),
            ToastVariant::Error,
        ));

        assert_ne!(info_id, error_id);

        let pending = manager.drain_pending();
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].message, "Same message");
        assert_eq!(pending[0].variant, ToastVariant::Info);
        assert_eq!(pending[1].message, "Same message");
        assert_eq!(pending[1].variant, ToastVariant::Error);
    }

    #[test]
    fn test_push_caps_repeat_count_at_999() {
        let mut manager = ToastManager::new();
        let first_id = manager.push(make_test_toast("Burst", Some(3000)));

        for _ in 0..1500 {
            let id = manager.push(make_test_toast("Burst", Some(3000)));
            assert_eq!(id, first_id);
        }

        let pending = manager.drain_pending();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].message, "Burst (x999)");
    }

    #[test]
    fn test_push_enqueues_new_toast_when_coalesce_window_expires() {
        let mut manager = ToastManager::new();
        manager.coalesce_window = Duration::from_millis(1);

        let first_id = manager.push(make_test_toast("Windowed", Some(3000)));
        thread::sleep(Duration::from_millis(10));
        let second_id = manager.push(make_test_toast("Windowed", Some(3000)));

        assert_ne!(first_id, second_id);

        let pending = manager.drain_pending();
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].message, "Windowed");
        assert_eq!(pending[1].message, "Windowed");
    }
}
