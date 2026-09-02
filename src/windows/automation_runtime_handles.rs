//! Exact runtime window handle registry for automation dispatch.
//!
//! Maps automation window IDs (e.g. `"agentChatDetached:thread-2"`) to live GPUI
//! [`AnyWindowHandle`] values, so that [`dispatch_gpui_event`] can target a
//! specific window without collapsing back to a shared [`WindowRole`].
//!
//! The automation *metadata* registry (`automation_registry.rs`) stores
//! [`AutomationWindowInfo`] for discovery and targeting.  This module stores
//! the *runtime handle* that GPUI needs to actually deliver events.
//!
//! # Lifecycle
//!
//! - **Upsert** when a window is created and its automation ID is known.
//! - **Remove** when the window closes.
//! - **Validate** with [`get_valid_runtime_window_handle`] before dispatch;
//!   stale handles are evicted automatically.

use gpui::{AnyWindowHandle, App};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::LazyLock;

#[derive(Clone, Copy)]
struct RuntimeWindowHandleEntry {
    handle: AnyWindowHandle,
    generation: Option<u64>,
    host_policy: Option<crate::runtime_policy::WindowHostPolicy>,
    app_instance: Option<u64>,
    theme_revision: u64,
    invalidation_epoch: u64,
    suppress_next_theme_notification: bool,
}

static RUNTIME_WINDOW_HANDLES: LazyLock<Mutex<HashMap<String, RuntimeWindowHandleEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static RUNTIME_REGISTRY_REVISION: AtomicU64 = AtomicU64::new(0);

static NEXT_RUNTIME_APP_INSTANCE: AtomicU64 = AtomicU64::new(1);

struct RuntimeWindowCloseObserver {
    app_instance: u64,
    _subscription: gpui::Subscription,
}

impl gpui::Global for RuntimeWindowCloseObserver {}

fn ensure_window_close_observer(cx: &mut App) -> anyhow::Result<u64> {
    if cx.has_global::<RuntimeWindowCloseObserver>() {
        return Ok(cx.global::<RuntimeWindowCloseObserver>().app_instance);
    }
    let app_instance = NEXT_RUNTIME_APP_INSTANCE
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
            value.checked_add(1)
        })
        .map_err(|_| anyhow::anyhow!("runtime_app_identity_exhausted"))?;
    let subscription = cx.on_window_closed(move |_, closed| {
        let lifetime = RUNTIME_WINDOW_HANDLES
            .lock()
            .iter()
            .find_map(|(id, entry)| {
                if entry.app_instance == Some(app_instance) && entry.handle.window_id() == closed {
                    entry.generation.map(|generation| (id.clone(), generation))
                } else {
                    None
                }
            });
        if let Some((id, generation)) = lifetime {
            remove_runtime_window_instance(&id, generation);
        }
    });
    cx.set_global(RuntimeWindowCloseObserver {
        app_instance,
        _subscription: subscription,
    });
    Ok(app_instance)
}

pub fn runtime_window_registry_revision() -> u64 {
    RUNTIME_REGISTRY_REVISION.load(Ordering::Acquire)
}

/// Only verified native lifetimes appear here; metadata-only entries cannot
/// silently acquire evaluation authority.
pub fn runtime_window_instances() -> Vec<(String, u64, crate::runtime_policy::WindowHostPolicy)> {
    RUNTIME_WINDOW_HANDLES
        .lock()
        .iter()
        .filter_map(|(id, entry)| Some((id.clone(), entry.generation?, entry.host_policy?)))
        .collect()
}

/// Register or update the GPUI window handle for an automation window ID.
pub fn upsert_runtime_window_handle(id: impl Into<String>, handle: AnyWindowHandle) {
    upsert_runtime_window_handle_instance(id, handle, None);
}

pub fn upsert_runtime_window_handle_instance(
    id: impl Into<String>,
    handle: AnyWindowHandle,
    generation: Option<u64>,
) {
    let id = id.into();
    RUNTIME_WINDOW_HANDLES.lock().insert(
        id.clone(),
        RuntimeWindowHandleEntry {
            handle,
            generation,
            host_policy: None,
            app_instance: None,
            theme_revision: 0,
            invalidation_epoch: 0,
            suppress_next_theme_notification: false,
        },
    );
    RUNTIME_REGISTRY_REVISION.fetch_add(1, Ordering::Release);
    tracing::info!(
        target: "script_kit::automation",
        window_id = %id,
        generation = ?generation,
        "automation.runtime_handle_upserted"
    );
}

/// Remove the runtime handle for an automation window ID.
///
/// Returns `true` if a handle was present and removed.
pub fn remove_runtime_window_handle(id: &str) -> bool {
    let removed = RUNTIME_WINDOW_HANDLES.lock().remove(id).is_some();
    if removed {
        RUNTIME_REGISTRY_REVISION.fetch_add(1, Ordering::Release);
        tracing::info!(
            target: "script_kit::automation",
            window_id = %id,
            "automation.runtime_handle_removed"
        );
    }
    removed
}

pub fn remove_runtime_window_handle_if_generation(id: &str, generation: u64) -> bool {
    let mut handles = RUNTIME_WINDOW_HANDLES.lock();
    if handles.get(id).and_then(|entry| entry.generation) != Some(generation) {
        return false;
    }
    let removed = handles.remove(id).is_some();
    if removed {
        RUNTIME_REGISTRY_REVISION.fetch_add(1, Ordering::Release);
        tracing::info!(
            target: "script_kit::automation",
            window_id = %id,
            generation,
            "automation.runtime_handle_generation_removed"
        );
    }
    removed
}

/// Get the runtime handle for an automation window ID without validation.
pub fn get_runtime_window_handle(id: &str) -> Option<AnyWindowHandle> {
    RUNTIME_WINDOW_HANDLES
        .lock()
        .get(id)
        .map(|entry| entry.handle)
}

pub fn get_runtime_window_handle_for_generation(
    id: &str,
    generation: u64,
) -> Option<AnyWindowHandle> {
    RUNTIME_WINDOW_HANDLES
        .lock()
        .get(id)
        .filter(|entry| entry.generation == Some(generation))
        .map(|entry| entry.handle)
}

/// Get the runtime handle for an automation window ID, validating that it
/// still refers to a live GPUI window.  Stale handles are evicted.
pub fn get_valid_runtime_window_handle(id: &str, cx: &mut App) -> Option<AnyWindowHandle> {
    let handle = get_runtime_window_handle(id)?;
    match handle.update(cx, |_, _, _| {}) {
        Ok(_) => Some(handle),
        Err(_) => {
            remove_runtime_window_handle(id);
            tracing::warn!(
                target: "script_kit::automation",
                window_id = %id,
                "automation.runtime_handle_stale"
            );
            None
        }
    }
}

pub fn get_valid_runtime_window_handle_for_generation(
    id: &str,
    generation: u64,
    cx: &mut App,
) -> Option<AnyWindowHandle> {
    let handle = get_runtime_window_handle_for_generation(id, generation)?;
    match handle.update(cx, |_, _, _| {}) {
        Ok(_) => Some(handle),
        Err(_) => {
            remove_runtime_window_handle_if_generation(id, generation);
            tracing::warn!(
                target: "script_kit::automation",
                window_id = %id,
                generation,
                "automation.runtime_handle_generation_stale"
            );
            None
        }
    }
}

/// Pair metadata and the real GPUI lifetime. All host checks precede publication.
pub fn register_runtime_window_instance(
    mut info: crate::protocol::AutomationWindowInfo,
    handle: AnyWindowHandle,
    cx: &mut App,
) -> anyhow::Result<crate::protocol::AutomationWindowInfo> {
    use crate::runtime_policy::WindowHostPolicy;
    let hidden = handle.update(cx, |_, window, _| window.is_owned_hidden())?;
    let policy = if hidden {
        WindowHostPolicy::OwnedHidden
    } else {
        WindowHostPolicy::Interactive
    };
    policy.validate()?;
    anyhow::ensure!(
        !hidden || (!info.visible && !info.focused),
        "owned_window_visible_metadata"
    );
    anyhow::ensure!(
        info.pid.is_none_or(|pid| pid == std::process::id()),
        "window_process_mismatch"
    );
    info.pid = Some(std::process::id());
    let app_instance = ensure_window_close_observer(cx)?;
    let mut handles = RUNTIME_WINDOW_HANDLES.lock();
    anyhow::ensure!(
        !handles.contains_key(&info.id),
        "runtime_window_already_registered"
    );
    anyhow::ensure!(
        handles.values().all(|entry| entry.handle != handle),
        "runtime_handle_already_registered"
    );
    if let Some(parent_id) = info.parent_window_id.as_deref() {
        let parent = handles
            .get(parent_id)
            .ok_or_else(|| anyhow::anyhow!("parent_runtime_missing"))?;
        anyhow::ensure!(
            parent.app_instance == Some(app_instance),
            "parent_app_instance_mismatch"
        );
        let parent_generation = parent
            .generation
            .filter(|generation| *generation > 0)
            .ok_or_else(|| anyhow::anyhow!("parent_runtime_generation_missing"))?;
        anyhow::ensure!(
            info.parent_window_generation
                .is_none_or(|expected| expected == parent_generation),
            "stale_parent_runtime"
        );
        anyhow::ensure!(
            parent.host_policy == Some(policy),
            "parent_host_policy_mismatch"
        );
        info.parent_window_generation = Some(parent_generation);
    }
    let info = super::automation_registry::insert_window_instance(info)?;
    RUNTIME_REGISTRY_REVISION.fetch_add(1, Ordering::Release);
    handles.insert(
        info.id.clone(),
        RuntimeWindowHandleEntry {
            handle,
            generation: info.generation,
            host_policy: Some(policy),
            app_instance: Some(app_instance),
            theme_revision: 0,
            invalidation_epoch: 0,
            suppress_next_theme_notification: false,
        },
    );
    Ok(info)
}

/// A deferred callback can remove only the exact lifetime it captured.
pub fn remove_runtime_window_instance(id: &str, generation: u64) -> bool {
    if generation == 0 {
        return false;
    }
    let mut handles = RUNTIME_WINDOW_HANDLES.lock();
    if handles.get(id).and_then(|entry| entry.generation) != Some(generation) {
        return false;
    }
    if super::automation_registry::remove_automation_window_if_generation(id, generation).is_none()
    {
        return false;
    }
    handles.remove(id);
    RUNTIME_REGISTRY_REVISION.fetch_add(1, Ordering::Release);
    true
}

pub fn runtime_window_host_policy(
    id: &str,
    generation: u64,
) -> anyhow::Result<crate::runtime_policy::WindowHostPolicy> {
    RUNTIME_WINDOW_HANDLES
        .lock()
        .get(id)
        .filter(|entry| generation > 0 && entry.generation == Some(generation))
        .and_then(|entry| entry.host_policy)
        .ok_or_else(|| anyhow::anyhow!("unverified_runtime_window_policy"))
}

pub(super) fn runtime_window_handles() -> Vec<AnyWindowHandle> {
    RUNTIME_WINDOW_HANDLES
        .lock()
        .values()
        .map(|entry| entry.handle)
        .collect()
}

/// Causal negative control: omit exactly one delivery, not the later frame draw.
pub fn suppress_next_theme_notification(id: &str, generation: u64) -> anyhow::Result<()> {
    anyhow::ensure!(
        crate::runtime_policy::is_owned_evaluation(),
        "owned_fault_control_required"
    );
    let mut handles = RUNTIME_WINDOW_HANDLES.lock();
    let entry = handles
        .get_mut(id)
        .filter(|entry| entry.generation == Some(generation))
        .ok_or_else(|| anyhow::anyhow!("stale_fault_target"))?;
    entry.suppress_next_theme_notification = true;
    Ok(())
}

pub(super) fn should_deliver_theme(handle: AnyWindowHandle) -> bool {
    let mut handles = RUNTIME_WINDOW_HANDLES.lock();
    let mut suppressed = false;
    for entry in handles.values_mut().filter(|entry| entry.handle == handle) {
        suppressed |= std::mem::take(&mut entry.suppress_next_theme_notification);
    }
    !suppressed
}

pub(super) fn record_theme_delivery(handle: AnyWindowHandle, revision: u64) {
    let mut handles = RUNTIME_WINDOW_HANDLES.lock();
    for entry in handles.values_mut().filter(|entry| entry.handle == handle) {
        entry.theme_revision = revision;
        entry.invalidation_epoch = entry.invalidation_epoch.saturating_add(1);
    }
}

/// Read-only delivery journal. No inspection or paint can create these entries.
pub fn theme_invalidations(revision: u64) -> Vec<crate::protocol::ThemeInvalidation> {
    let handles = RUNTIME_WINDOW_HANDLES.lock();
    let mut result: Vec<_> = handles
        .iter()
        .filter_map(|(id, entry)| {
            let generation = entry.generation?;
            (entry.theme_revision == revision && entry.invalidation_epoch > 0).then(|| {
                crate::protocol::ThemeInvalidation {
                    target: crate::protocol::AutomationWindowTarget::Instance {
                        id: id.clone(),
                        generation,
                    },
                    revision,
                    cause: crate::protocol::ThemeInvalidationCause::ThemePublication,
                    invalidation_epoch: entry.invalidation_epoch,
                }
            })
        })
        .collect();
    result.sort_by(|left, right| match (&left.target, &right.target) {
        (
            crate::protocol::AutomationWindowTarget::Instance { id: left, .. },
            crate::protocol::AutomationWindowTarget::Instance { id: right, .. },
        ) => left.cmp(right),
        _ => std::cmp::Ordering::Equal,
    });
    result
}

thread_local! {
    static LOCAL_DISPATCH_TARGET: std::cell::Cell<Option<AnyWindowHandle>> = const { std::cell::Cell::new(None) };
}

/// Scope actual GPUI event delivery, never a global input or visibility override.
pub fn with_runtime_window_dispatch<R>(handle: AnyWindowHandle, dispatch: impl FnOnce() -> R) -> R {
    struct Restore(Option<AnyWindowHandle>);
    impl Drop for Restore {
        fn drop(&mut self) {
            LOCAL_DISPATCH_TARGET.set(self.0);
        }
    }
    let _restore = Restore(LOCAL_DISPATCH_TARGET.replace(Some(handle)));
    dispatch()
}

pub fn accepts_main_window_input(window: &gpui::Window) -> bool {
    if window.is_owned_hidden() {
        let handle = window.window_handle();
        return LOCAL_DISPATCH_TARGET.get() == Some(handle)
            && get_runtime_window_handle("main") == Some(handle);
    }
    crate::is_main_window_visible()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct RegistryTestView;

    impl gpui::Render for RegistryTestView {
        fn render(
            &mut self,
            _: &mut gpui::Window,
            _: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            gpui::div()
        }
    }

    #[gpui::test]
    fn paired_lifetime_follows_actual_close_and_reopen(cx: &mut gpui::TestAppContext) {
        use gpui::AppContext as _;
        let _guard = crate::windows::automation_registry::tests::registry_guard();
        let info = crate::protocol::AutomationWindowInfo {
            id: "paired-runtime-close-test".into(),
            kind: crate::protocol::AutomationWindowKind::Main,
            title: None,
            focused: false,
            visible: false,
            semantic_surface: None,
            bounds: None,
            parent_window_id: None,
            parent_window_generation: None,
            parent_kind: None,
            generation: None,
            pid: None,
        };
        let first = cx.update(|cx| {
            let handle = cx
                .open_window(
                    gpui::WindowOptions {
                        show: false,
                        focus: false,
                        ..Default::default()
                    },
                    |_, cx| cx.new(|_| RegistryTestView),
                )
                .unwrap();
            let registered =
                register_runtime_window_instance(info.clone(), handle.into(), cx).unwrap();
            (handle, registered.generation.unwrap())
        });
        cx.update(|cx| {
            first
                .0
                .update(cx, |_, window, _| window.remove_window())
                .unwrap()
        });
        cx.run_until_parked();
        assert!(get_runtime_window_handle(&info.id).is_none());
        assert!(crate::windows::automation_window_by_id(&info.id).is_none());
        let second = cx.update(|cx| {
            let handle = cx
                .open_window(
                    gpui::WindowOptions {
                        show: false,
                        focus: false,
                        ..Default::default()
                    },
                    |_, cx| cx.new(|_| RegistryTestView),
                )
                .unwrap();
            let registered =
                register_runtime_window_instance(info.clone(), handle.into(), cx).unwrap();
            (handle, registered.generation.unwrap())
        });
        assert!(second.1 > first.1);
        assert!(!remove_runtime_window_instance(&info.id, first.1));
        assert_eq!(
            get_runtime_window_handle_for_generation(&info.id, second.1),
            Some(second.0.into())
        );
        cx.update(|cx| {
            second
                .0
                .update(cx, |_, window, _| window.remove_window())
                .unwrap()
        });
        cx.run_until_parked();
        assert!(get_runtime_window_handle(&info.id).is_none());
        assert!(crate::windows::automation_window_by_id(&info.id).is_none());
    }

    #[test]
    fn remove_returns_false_when_absent() {
        assert!(!remove_runtime_window_handle("nonexistent_rt_handle_test"));
    }
}
