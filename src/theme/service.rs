//! Global Theme Service
//!
//! Provides a singleton theme watcher that broadcasts changes to all windows,
//! replacing per-window theme watchers. This eliminates duplicate watchers
//! and ensures consistent theme updates across the entire application.
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::theme::service::ensure_theme_service;
//!
//! // Call once at app startup or before opening any window
//! ensure_theme_service(cx);
//! ```
//!
//! The service will:
//! 1. Watch ~/.scriptkit/theme.json for changes
//! 2. Sync gpui-component theme when changes are detected
//! 3. Notify all registered windows to re-render
//!
//! # Architecture
//!
//! - Uses AtomicBool to ensure only one watcher runs
//! - Uses the WindowRegistry to notify all windows
//! - Polls for changes every 200ms (same as previous per-window watchers)

use std::sync::atomic::{AtomicBool, Ordering};

use gpui::App;
use tracing::{debug, info, warn};

use super::types::AppearanceMode;
use crate::watcher::ThemeWatcher;
use crate::windows;

const FAST_POLL_MS: u64 = 200;
const MEDIUM_POLL_MS: u64 = 500;
const SLOW_POLL_MS: u64 = 2000;
const FAST_POLL_IDLE_CUTOFF: u64 = 5;
const MEDIUM_POLL_IDLE_CUTOFF: u64 = 10;

/// Flag to track if the theme service is running
static THEME_SERVICE_RUNNING: AtomicBool = AtomicBool::new(false);

/// Reads the revision from the same immutable snapshot as the theme.
pub fn theme_revision() -> u64 {
    super::get_theme_snapshot().revision
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemePublicationSource {
    Startup,
    FileReload,
    Appearance,
    LivePreview,
    Revert,
    ChooserPreview { sync_native: bool },
    Persisted,
}

impl ThemePublicationSource {
    fn label(self) -> &'static str {
        match self {
            Self::Startup => "startup",
            Self::FileReload => "file_reload",
            Self::Appearance => "appearance",
            Self::LivePreview => "live_preview",
            Self::Revert => "revert",
            Self::ChooserPreview { .. } => "chooser_preview",
            Self::Persisted => "persisted",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ThemePublishError {
    #[error("stale_theme_revision: expected {expected}, actual {actual}")]
    StaleRevision { expected: u64, actual: u64 },
    #[error("theme_revision_exhausted")]
    RevisionExhausted,
    #[error("no_theme_preview")]
    NoPreview,
    #[error(transparent)]
    InvalidEdit(#[from] super::live_edit::ThemeEditError),
    #[error("theme_storage_failed: {0}")]
    Storage(#[from] anyhow::Error),
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemePublicationReceipt {
    pub previous_revision: u64,
    pub revision: u64,
    pub source: ThemePublicationSource,
    /// Delivery is performed before returning; rendered frames are subsequent
    /// evidence and cannot roll back an already published snapshot.
    pub invalidation_requested: bool,
    pub native_refresh_requested: bool,
}

pub fn publish_runtime_theme(
    cx: &mut App,
    expected_revision: u64,
    prepared: super::live_edit::PreparedTheme,
    source: ThemePublicationSource,
) -> Result<ThemePublicationReceipt, ThemePublishError> {
    let snapshot = super::types::commit_prepared_theme(expected_revision, prepared, |component| {
        component.apply(cx)
    })?;
    windows::notify_all_windows(cx);
    let native_refresh_requested = !crate::runtime_policy::is_owned_evaluation()
        && !matches!(
            source,
            ThemePublicationSource::ChooserPreview { sync_native: false }
                | ThemePublicationSource::LivePreview
        );
    if native_refresh_requested {
        super::gpui_integration::sync_native_window_theme_for_theme(
            &snapshot.theme,
            source.label(),
        );
    }
    Ok(ThemePublicationReceipt {
        previous_revision: expected_revision,
        revision: snapshot.revision,
        source,
        invalidation_requested: true,
        native_refresh_requested,
    })
}

pub(crate) fn reload_theme(
    cx: &mut App,
    source: ThemePublicationSource,
) -> Result<ThemePublicationReceipt, ThemePublishError> {
    let expected = theme_revision();
    let prepared = super::live_edit::prepare_theme(super::types::try_load_theme()?)?;
    // The watcher also sees our own atomic save. Do not turn an identical
    // echo into a foreign publication that invalidates the chooser baseline.
    // Appearance updates still refresh the bridge even with equal file bytes.
    if source == ThemePublicationSource::FileReload
        && serde_json::to_value(prepared.theme.as_ref()).map_err(anyhow::Error::from)?
            == serde_json::to_value(super::get_theme_snapshot().theme.as_ref())
                .map_err(anyhow::Error::from)?
    {
        return Ok(ThemePublicationReceipt {
            previous_revision: expected,
            revision: expected,
            source,
            invalidation_requested: false,
            native_refresh_requested: false,
        });
    }
    publish_runtime_theme(cx, expected, prepared, source)
}

/// Startup alone permits the historical missing/invalid-file default. Every
/// later reload is fallible and keeps the last known good snapshot.
pub fn initialize_theme(cx: &mut App) -> Result<ThemePublicationReceipt, ThemePublishError> {
    let expected = theme_revision();
    publish_runtime_theme(
        cx,
        expected,
        super::live_edit::prepare_theme(super::load_theme())?,
        ThemePublicationSource::Startup,
    )
}

#[allow(dead_code)]
pub(crate) fn reapply_runtime_theme_overrides(cx: &mut App) {
    if let Err(error) = reload_theme(cx, ThemePublicationSource::FileReload) {
        warn!(%error, "Runtime theme reload retained last known good theme");
    }
}

fn should_reload_theme(
    file_changed: bool,
    appearance_changed: bool,
    auto_appearance: bool,
) -> bool {
    file_changed || (appearance_changed && auto_appearance)
}

fn theme_poll_detect_appearance_change_if_auto(
    auto_appearance: bool,
    last_system_dark: &mut bool,
    detect_system_appearance: impl FnOnce() -> bool,
) -> bool {
    if !auto_appearance {
        return false;
    }

    let current_system_dark = detect_system_appearance();
    let appearance_changed = current_system_dark != *last_system_dark;
    *last_system_dark = current_system_dark;
    appearance_changed
}

fn theme_poll_refresh_baseline_on_auto_enable(
    previous_auto_appearance: bool,
    auto_appearance: bool,
    last_system_dark: &mut bool,
    detect_system_appearance: impl FnOnce() -> bool,
) {
    if !previous_auto_appearance && auto_appearance {
        *last_system_dark = detect_system_appearance();
    }
}

fn drain_pending_events<T>(rx: &std::sync::mpsc::Receiver<T>) -> bool {
    let mut has_events = false;
    while let Ok(_evt) = rx.try_recv() {
        has_events = true;
    }
    has_events
}

struct ThemeServiceLifetime;

impl Drop for ThemeServiceLifetime {
    fn drop(&mut self) {
        THEME_SERVICE_RUNNING.store(false, Ordering::SeqCst);
        info!("Theme service stopped");
    }
}

/// Ensure the global theme service is running.
///
/// This is idempotent - calling it multiple times is safe and will only
/// start one watcher. The watcher runs until the application shuts down.
///
/// # Arguments
/// * `cx` - The GPUI App context
pub fn ensure_theme_service(cx: &mut App) {
    if crate::runtime_policy::is_owned_evaluation() {
        return;
    }
    // Use swap to atomically check and set in one operation
    if THEME_SERVICE_RUNNING.swap(true, Ordering::SeqCst) {
        // Already running
        return;
    }

    info!("Starting global theme service");
    let lifetime = ThemeServiceLifetime;

    cx.spawn(async move |cx: &mut gpui::AsyncApp| {
        let _lifetime = lifetime;
        let (mut watcher, rx) = ThemeWatcher::new();

        if let Err(error) = watcher.start() {
            warn!(error = ?error, "Failed to start theme file watcher");
            return;
        }

        info!("Theme file watcher started successfully");

        // Adaptive polling: starts at 200ms, increases to 2s when idle
        let mut idle_count = 0u32;
        let mut auto_appearance = matches!(
            super::types::get_cached_theme().appearance,
            AppearanceMode::Auto
        );
        let mut last_system_dark = false;
        if auto_appearance {
            last_system_dark = super::types::detect_system_appearance();
        }
        loop {
            // Adaptive polling: 200ms when active, up to 2000ms when idle
            let poll_interval = if u64::from(idle_count) < FAST_POLL_IDLE_CUTOFF {
                FAST_POLL_MS
            } else if u64::from(idle_count) < MEDIUM_POLL_IDLE_CUTOFF {
                MEDIUM_POLL_MS
            } else {
                SLOW_POLL_MS
            };
            cx.background_executor()
                .timer(std::time::Duration::from_millis(poll_interval))
                .await;

            let file_changed = drain_pending_events(&rx);
            let appearance_changed = theme_poll_detect_appearance_change_if_auto(
                auto_appearance,
                &mut last_system_dark,
                super::types::detect_system_appearance,
            );

            if should_reload_theme(file_changed, appearance_changed, auto_appearance) {
                idle_count = 0; // Reset on activity

                if file_changed {
                    info!("Theme changed, syncing to all windows");
                }

                let update_result = cx.update(|cx| {
                    reload_theme(cx, if file_changed { ThemePublicationSource::FileReload } else { ThemePublicationSource::Appearance })
                        .map(|_| matches!(super::get_theme_snapshot().theme.appearance, AppearanceMode::Auto))
                });

                let updated_auto_appearance = match update_result {
                    Ok(updated_auto_appearance) => updated_auto_appearance,
                    Err(error) => {
                        warn!(error = ?error, "Theme reload failed; retaining last known good publication");
                        continue;
                    }
                };

                if auto_appearance != updated_auto_appearance {
                    debug!(
                        previous_auto_appearance = auto_appearance,
                        auto_appearance = updated_auto_appearance,
                        "Theme appearance mode updated"
                    );
                }

                theme_poll_refresh_baseline_on_auto_enable(
                    auto_appearance,
                    updated_auto_appearance,
                    &mut last_system_dark,
                    super::types::detect_system_appearance,
                );
                auto_appearance = updated_auto_appearance;
            } else {
                idle_count = idle_count.saturating_add(1);
            }
        }
    })
    .detach();
}

/// Persist a theme to disk, reload the global cache, sync gpui-component + native
/// window state, bump the theme revision, and notify every open window — all in
/// one atomic step.
///
/// Use this from the theme chooser Apply path (and any other code that wants to
/// commit a theme change app-wide) instead of writing to disk and hoping the
/// file-watcher picks it up.
#[allow(dead_code)] // Called from render_builtins/theme_chooser.rs (include!() binary target)
pub(crate) fn persist_theme_and_sync_all_windows(
    cx: &mut App,
    theme: &crate::theme::Theme,
    source: &'static str,
) -> anyhow::Result<super::types::Theme> {
    let expected = theme_revision();
    let prepared = super::live_edit::prepare_theme(theme.clone())?;
    crate::theme::presets::write_theme_to_disk(&prepared.theme)?;
    let receipt = publish_runtime_theme(cx, expected, prepared, ThemePublicationSource::Persisted)?;
    info!(
        source,
        revision = receipt.revision,
        "theme_persisted_and_synced_all_windows"
    );
    Ok(super::get_cached_theme())
}

/// Check if the theme service is currently running.
///
/// Mainly useful for debugging/testing.
#[cfg(test)]
fn is_theme_service_running() -> bool {
    THEME_SERVICE_RUNNING.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_service_flag() {
        // Reset flag for test
        THEME_SERVICE_RUNNING.store(false, Ordering::SeqCst);

        assert!(!is_theme_service_running());

        // Manually set flag (since we can't run actual service in unit test)
        THEME_SERVICE_RUNNING.store(true, Ordering::SeqCst);
        assert!(is_theme_service_running());

        // Clean up
        THEME_SERVICE_RUNNING.store(false, Ordering::SeqCst);
    }

    #[test]
    fn test_theme_revision_starts_at_one() {
        // Revision starts at 1 (not 0) so initial comparison fails
        let rev = theme_revision();
        assert!(rev >= 1);
    }

    #[test]
    fn test_decode_theme_json_reports_errors_and_warnings_for_invalid_theme_json() {
        let invalid_theme_json = serde_json::json!({
            "colors": {
                "background": {
                    "main": -1
                }
            },
            "unexpected_key": true
        });

        let error = super::super::types::decode_theme_json(&invalid_theme_json.to_string(), true)
            .expect_err("invalid color must prevent theme loading")
            .to_string();

        assert!(error.contains("[WARN] /unexpected_key:"));
        assert!(error.contains("[ERROR] /colors/background/main:"));
    }

    #[test]
    fn test_decode_theme_json_loads_valid_theme_json() {
        let valid_theme_json = serde_json::json!({
            "colors": {
                "background": {
                    "main": "#111111"
                }
            },
            "vibrancy": {
                "enabled": true,
                "material": "hud"
            }
        });

        let theme = super::super::types::decode_theme_json(&valid_theme_json.to_string(), true)
            .expect("valid theme must load");

        assert_eq!(theme.colors.background.main, 0x111111);
    }

    #[test]
    fn test_should_reload_theme_when_file_changes() {
        assert!(should_reload_theme(true, false, false));
    }

    #[test]
    fn test_should_reload_theme_when_appearance_changes_in_auto_mode() {
        assert!(should_reload_theme(false, true, true));
    }

    #[test]
    fn test_should_not_reload_theme_when_appearance_changes_outside_auto_mode() {
        assert!(!should_reload_theme(false, true, false));
    }

    #[test]
    fn test_theme_poll_detect_appearance_change_if_auto_skips_detection_when_not_auto() {
        let mut last_system_dark = true;
        let mut called = false;

        let appearance_changed =
            theme_poll_detect_appearance_change_if_auto(false, &mut last_system_dark, || {
                called = true;
                false
            });

        assert!(!appearance_changed);
        assert!(!called);
        assert!(last_system_dark);
    }

    #[test]
    fn test_theme_poll_detect_appearance_change_if_auto_updates_last_system_state() {
        let mut last_system_dark = true;

        let appearance_changed =
            theme_poll_detect_appearance_change_if_auto(true, &mut last_system_dark, || false);

        assert!(appearance_changed);
        assert!(!last_system_dark);
    }

    #[test]
    fn test_theme_poll_refresh_baseline_on_auto_enable_detects_baseline_once() {
        let mut last_system_dark = false;
        let mut call_count = 0usize;

        theme_poll_refresh_baseline_on_auto_enable(false, true, &mut last_system_dark, || {
            call_count += 1;
            true
        });

        assert_eq!(call_count, 1);
        assert!(last_system_dark);
    }

    #[test]
    fn test_theme_poll_refresh_baseline_on_auto_enable_skips_when_auto_not_newly_enabled() {
        let mut last_system_dark = false;
        let mut call_count = 0usize;

        theme_poll_refresh_baseline_on_auto_enable(true, true, &mut last_system_dark, || {
            call_count += 1;
            true
        });

        assert_eq!(call_count, 0);
        assert!(!last_system_dark);
    }

    #[test]
    fn test_drain_pending_events_returns_false_when_queue_is_empty() {
        let (_tx, rx) = std::sync::mpsc::channel::<u8>();
        assert!(!drain_pending_events(&rx));
    }

    #[test]
    fn test_drain_pending_events_drains_all_queued_events() {
        let (tx, rx) = std::sync::mpsc::channel::<u8>();
        tx.send(1)
            .expect("sending first queued event for drain test should succeed");
        tx.send(2)
            .expect("sending second queued event for drain test should succeed");
        tx.send(3)
            .expect("sending third queued event for drain test should succeed");

        assert!(drain_pending_events(&rx));
        assert!(matches!(
            rx.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
    }
}
