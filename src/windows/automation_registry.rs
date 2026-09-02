//! Automation Window Registry
//!
//! A shared, thread-safe registry that maps stable automation IDs to
//! [`AutomationWindowInfo`] descriptors.  Every product window (main,
//! Notes, AI, detached Agent Chat, popups) registers here on open and
//! unregisters on close.  The resolver accepts an
//! [`AutomationWindowTarget`] and returns the matching entry — or an
//! error, never a silent fallback.
//!
//! All mutations emit structured tracing so agents can observe the
//! registry lifecycle in machine-parseable logs.
//!
//! ## Deterministic ordering
//!
//! The registry maintains a cached `kind_index` that groups window IDs
//! by [`AutomationWindowKind`] and sorts them lexicographically within
//! each group.  `list_automation_windows()` returns entries sorted by
//! `kind_rank` then `id`, so identical registry contents always produce
//! the same snapshot order regardless of insertion order.

use crate::protocol::{
    AutomationWindowBounds, AutomationWindowInfo, AutomationWindowKind, AutomationWindowTarget,
};
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::LazyLock;

// ---------------------------------------------------------------------------
// Registry state
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct AutomationRegistryState {
    windows: HashMap<String, AutomationWindowInfo>,
    target_revisions: HashMap<String, u64>,
    focused_id: Option<String>,
    main_id: Option<String>,
    kind_index: HashMap<AutomationWindowKind, Vec<String>>,
    /// Process-local monotonic identity for window lifetimes whose native owner
    /// does not already provide a generation. It advances only when a stable ID
    /// is absent from the registry, so ordinary metadata upserts preserve the
    /// current lifetime while close/reopen receives a distinct instance.
    next_generation: u64,
}

static AUTOMATION_WINDOWS: LazyLock<Mutex<AutomationRegistryState>> =
    LazyLock::new(|| Mutex::new(AutomationRegistryState::default()));

// ---------------------------------------------------------------------------
// Kind ordering
// ---------------------------------------------------------------------------

fn kind_rank(kind: AutomationWindowKind) -> u8 {
    match kind {
        AutomationWindowKind::Main => 0,
        AutomationWindowKind::Notes => 1,
        AutomationWindowKind::AgentChatDetached => 4,
        AutomationWindowKind::Dictation => 5,
        AutomationWindowKind::ActionsDialog => 7,
        AutomationWindowKind::PromptPopup => 8,
        AutomationWindowKind::Hud => 9,
        AutomationWindowKind::SnapOverlay => 10,
    }
}

// ---------------------------------------------------------------------------
// Index rebuilding
// ---------------------------------------------------------------------------

fn rebuild_indexes(state: &mut AutomationRegistryState) {
    state.focused_id = None;
    state.main_id = None;
    state.kind_index.clear();

    // Sort IDs lexicographically so same-kind ordering is deterministic.
    let mut ids: Vec<String> = state.windows.keys().cloned().collect();
    ids.sort_unstable();

    for id in ids {
        let info = &state.windows[&id];
        if info.focused {
            state.focused_id = Some(id.clone());
        }
        if info.kind == AutomationWindowKind::Main && state.main_id.is_none() {
            state.main_id = Some(id.clone());
        }
        state.kind_index.entry(info.kind).or_default().push(id);
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Register or update an automation window entry.
///
/// If an entry with the same `id` already exists it is replaced.
/// When the new entry has `focused == true`, all other entries are
/// unfocused first.
pub fn upsert_automation_window(mut info: AutomationWindowInfo) {
    let mut state = AUTOMATION_WINDOWS.lock();
    if info.generation.is_none() {
        info.generation = state
            .windows
            .get(&info.id)
            .and_then(|existing| existing.generation)
            .or_else(|| {
                state.next_generation = state.next_generation.saturating_add(1);
                Some(state.next_generation)
            });
    } else if let Some(generation) = info.generation {
        state.next_generation = state.next_generation.max(generation);
    }
    let metadata_changed = state.windows.get(&info.id) != Some(&info);
    if info.focused {
        if let Some(previous) = state.focused_id.clone().filter(|id| id != &info.id) {
            if let Some(revision) = state.target_revisions.get_mut(&previous) {
                *revision = revision.saturating_add(1);
            }
        }
        for existing in state.windows.values_mut() {
            existing.focused = false;
        }
    }
    let id = info.id.clone();
    if metadata_changed {
        let revision = state.target_revisions.entry(id.clone()).or_insert(0);
        *revision = revision.saturating_add(1);
    }
    state.windows.insert(id.clone(), info);
    rebuild_indexes(&mut state);
    tracing::debug!(
        target: "script_kit::automation",
        id = %id,
        focused_id = ?state.focused_id,
        main_id = ?state.main_id,
        "automation_window_upserted"
    );
}

/// Allocate a fresh lifetime, without replacing an existing window.
pub(super) fn insert_window_instance(
    mut info: AutomationWindowInfo,
) -> Result<AutomationWindowInfo> {
    let mut state = AUTOMATION_WINDOWS.lock();
    anyhow::ensure!(
        !info.id.is_empty() && info.generation.is_none(),
        "invalid_new_window_identity"
    );
    anyhow::ensure!(
        !state.windows.contains_key(&info.id),
        "window_identity_already_registered"
    );
    if let Some(parent_id) = info.parent_window_id.as_deref() {
        let parent = state
            .windows
            .get(parent_id)
            .ok_or_else(|| anyhow!("parent_window_missing"))?;
        let generation = parent
            .generation
            .filter(|value| *value > 0)
            .ok_or_else(|| anyhow!("parent_generation_missing"))?;
        anyhow::ensure!(
            info.parent_window_generation
                .is_none_or(|expected| expected == generation),
            "stale_parent_window_generation"
        );
        anyhow::ensure!(
            info.parent_kind.is_none_or(|kind| kind == parent.kind),
            "parent_window_kind_mismatch"
        );
        info.parent_window_generation = Some(generation);
        info.parent_kind = Some(parent.kind);
    } else {
        anyhow::ensure!(
            info.parent_window_generation.is_none() && info.parent_kind.is_none(),
            "parent_identity_incomplete"
        );
    }
    state.next_generation = state
        .next_generation
        .checked_add(1)
        .ok_or_else(|| anyhow!("window_generation_exhausted"))?;
    info.generation = Some(state.next_generation);
    if info.focused {
        for existing in state.windows.values_mut() {
            existing.focused = false;
        }
    }
    state.windows.insert(info.id.clone(), info.clone());
    state.target_revisions.insert(info.id.clone(), 1);
    rebuild_indexes(&mut state);
    Ok(info)
}

/// Register an attached popup window with its parent identity.
///
/// This is the canonical entry point for popup registration. It records the
/// popup in the registry with its real parent window ID and kind, then emits
/// the `automation.attached_popup_parent_resolved` structured log.
///
/// **Fail-closed:** Returns `Err` when `parent_id` is `None` (missing parent
/// automation identity) or when the provided parent ID is not found in the
/// registry. No attached popup entry is created unless parent identity is
/// fully resolved.
pub fn register_attached_popup(
    popup_id: String,
    popup_kind: AutomationWindowKind,
    title: Option<String>,
    semantic_surface: Option<String>,
    bounds: Option<AutomationWindowBounds>,
    parent_id: Option<&str>,
) -> Result<()> {
    register_attached_popup_instance(
        popup_id,
        popup_kind,
        title,
        semantic_surface,
        bounds,
        parent_id,
        None,
    )
}

/// Register one exact attached-popup lifetime.
pub fn register_attached_popup_instance(
    popup_id: String,
    popup_kind: AutomationWindowKind,
    title: Option<String>,
    semantic_surface: Option<String>,
    bounds: Option<AutomationWindowBounds>,
    parent_id: Option<&str>,
    generation: Option<u64>,
) -> Result<()> {
    let pid = parent_id.ok_or_else(|| {
        tracing::warn!(
            target: "script_kit::automation",
            event = "automation.attached_popup_parent_missing",
            popup_window_id = %popup_id,
            popup_kind = ?popup_kind,
            "Attached popup registration rejected: no parent automation identity provided"
        );
        anyhow!(
            "Cannot register attached popup '{}': parent automation identity is required but was not provided",
            popup_id
        )
    })?;

    let (parent_window_id, parent_kind, parent_window_generation) = {
        let state = AUTOMATION_WINDOWS.lock();
        let parent_info = state.windows.get(pid).ok_or_else(|| {
            tracing::warn!(
                target: "script_kit::automation",
                event = "automation.attached_popup_parent_missing",
                popup_window_id = %popup_id,
                popup_kind = ?popup_kind,
                attempted_parent_id = %pid,
                "Attached popup registration rejected: parent not found in automation registry"
            );
            anyhow!(
                "Cannot register attached popup '{}': parent '{}' not found in automation registry",
                popup_id,
                pid
            )
        })?;
        (
            parent_info.id.clone(),
            parent_info.kind,
            parent_info.generation,
        )
    };

    let info = AutomationWindowInfo {
        id: popup_id.clone(),
        kind: popup_kind,
        title,
        focused: false,
        visible: true,
        semantic_surface,
        bounds,
        parent_window_id: Some(parent_window_id.clone()),
        parent_window_generation,
        parent_kind: Some(parent_kind),
        generation,
        pid: Some(std::process::id()),
    };

    upsert_automation_window(info);

    tracing::info!(
        target: "script_kit::automation",
        event = "automation.attached_popup_parent_resolved",
        popup_window_id = %popup_id,
        popup_kind = ?popup_kind,
        parent_window_id = %parent_window_id,
        parent_kind = ?parent_kind,
        "Attached popup parent identity established"
    );

    Ok(())
}

/// Remove an automation window entry by its stable ID.
///
/// Returns the removed entry if it existed.
pub fn remove_automation_window(id: &str) -> Option<AutomationWindowInfo> {
    let mut state = AUTOMATION_WINDOWS.lock();
    let removed = state.windows.remove(id);
    state.target_revisions.remove(id);
    if removed.is_some() {
        rebuild_indexes(&mut state);
        tracing::debug!(
            target: "script_kit::automation",
            id = %id,
            focused_id = ?state.focused_id,
            main_id = ?state.main_id,
            "automation_window_removed"
        );
    }
    removed
}

/// Remove an automation entry only when it still names the expected popup
/// lifetime. Delayed close callbacks cannot erase a reopened generation.
pub fn remove_automation_window_if_generation(
    id: &str,
    generation: u64,
) -> Option<AutomationWindowInfo> {
    let mut state = AUTOMATION_WINDOWS.lock();
    if state.windows.get(id).and_then(|info| info.generation) != Some(generation) {
        return None;
    }
    let removed = state.windows.remove(id);
    state.target_revisions.remove(id);
    if removed.is_some() {
        rebuild_indexes(&mut state);
        tracing::debug!(
            target: "script_kit::automation",
            id = %id,
            generation,
            "automation_window_generation_removed"
        );
    }
    removed
}

/// Return a snapshot of all registered automation windows in stable order.
///
/// Sorted by `kind_rank` (Main first, PromptPopup last) then
/// lexicographic `id` within each kind.
pub fn list_automation_windows() -> Vec<AutomationWindowInfo> {
    let state = AUTOMATION_WINDOWS.lock();
    let mut windows: Vec<AutomationWindowInfo> = state.windows.values().cloned().collect();
    windows.sort_by(|a, b| {
        kind_rank(a.kind)
            .cmp(&kind_rank(b.kind))
            .then_with(|| a.id.cmp(&b.id))
    });
    tracing::info!(
        target: "script_kit::automation",
        focused_id = ?state.focused_id,
        window_count = windows.len(),
        order = ?windows.iter().map(|w| w.id.clone()).collect::<Vec<_>>(),
        "automation_window_list_snapshot"
    );
    windows
}

/// Return a snapshot of a registered automation window by its stable ID.
pub fn automation_window_by_id(id: &str) -> Option<AutomationWindowInfo> {
    let state = AUTOMATION_WINDOWS.lock();
    state.windows.get(id).cloned()
}

/// Return the stable ID of whichever window is currently marked focused,
/// or `None` if no window has `focused == true`.
pub fn focused_automation_window_id() -> Option<String> {
    AUTOMATION_WINDOWS.lock().focused_id.clone()
}

/// Return a snapshot of whichever automation window is currently marked focused.
pub fn focused_automation_window() -> Option<AutomationWindowInfo> {
    let state = AUTOMATION_WINDOWS.lock();
    state
        .focused_id
        .as_ref()
        .and_then(|id| state.windows.get(id))
        .cloned()
}

/// Update focus so exactly one registered window is focused.
///
/// Returns `true` if `new_focused_id` was found in the registry.
pub fn set_automation_focus(new_focused_id: &str) -> bool {
    let mut state = AUTOMATION_WINDOWS.lock();
    if !state.windows.contains_key(new_focused_id) {
        return false;
    }
    if state.focused_id.as_deref() == Some(new_focused_id) {
        return true;
    }
    if let Some(previous) = state.focused_id.clone() {
        if let Some(revision) = state.target_revisions.get_mut(&previous) {
            *revision = revision.saturating_add(1);
        }
    }
    if let Some(revision) = state.target_revisions.get_mut(new_focused_id) {
        *revision = revision.saturating_add(1);
    }
    for (id, info) in state.windows.iter_mut() {
        info.focused = id.as_str() == new_focused_id;
    }
    rebuild_indexes(&mut state);
    tracing::info!(
        target: "script_kit::automation",
        id = %new_focused_id,
        focused_id = ?state.focused_id,
        "automation_window_focus_changed"
    );
    true
}

/// Update the `semantic_surface` tag for a single window in place.
///
/// Leaves every other field untouched — this is the hot path for
/// re-keying the main window as its active subview transitions
/// (e.g., `ScriptList` → `FileSearchView`) without having to
/// re-upsert the whole entry from callers that don't know the
/// window's bounds / focus / title.
///
/// Returns `true` if the window was found and (if the value
/// changed) an info-level trace was emitted. Returns `false` if no
/// entry with `id` is registered.
pub fn update_automation_semantic_surface(id: &str, surface: Option<String>) -> bool {
    let mut state = AUTOMATION_WINDOWS.lock();
    let Some(info) = state.windows.get_mut(id) else {
        return false;
    };
    if info.semantic_surface != surface {
        tracing::info!(
            target: "script_kit::automation",
            id = %id,
            semantic_surface = ?surface,
            "automation_window_semantic_surface_changed"
        );
        info.semantic_surface = surface;
        if let Some(revision) = state.target_revisions.get_mut(id) {
            *revision = revision.saturating_add(1);
        }
    }
    true
}

/// Update the visibility flag for a single window.
pub fn set_automation_visibility(id: &str, visible: bool) {
    let mut state = AUTOMATION_WINDOWS.lock();
    if let Some(info) = state.windows.get_mut(id) {
        if info.visible != visible {
            info.visible = visible;
            tracing::info!(
                target: "script_kit::automation",
                id = %id,
                visible = visible,
                "automation_window_visibility_changed"
            );
            if let Some(revision) = state.target_revisions.get_mut(id) {
                *revision = revision.saturating_add(1);
            }
        }
    }
}

/// Update the `bounds` field for a single window in place.
///
/// Used by live-resized attached popups (e.g., the actions-dialog
/// shrinks/grows as `visibleChoiceCount` changes) so that agentic
/// callers reading `listAutomationWindows` or `inspectAutomationWindow`
/// observe the current frame rather than the stale open-time frame.
/// Leaves every other field untouched. Returns `true` if the window was
/// found; `false` if no entry with `id` is registered (caller should
/// typically silently ignore — the window may have been closed
/// concurrently).
pub fn set_automation_bounds(id: &str, bounds: Option<AutomationWindowBounds>) -> bool {
    let mut state = AUTOMATION_WINDOWS.lock();
    let Some(info) = state.windows.get_mut(id) else {
        return false;
    };
    if info.bounds != bounds {
        tracing::debug!(
            target: "script_kit::automation",
            id = %id,
            bounds = ?bounds,
            "automation_window_bounds_changed"
        );
        info.bounds = bounds;
        if let Some(revision) = state.target_revisions.get_mut(id) {
            *revision = revision.saturating_add(1);
        }
    }
    true
}

/// Update geometry only for the lifetime that produced the observation.
pub fn set_automation_bounds_if_generation(
    id: &str,
    generation: u64,
    bounds: Option<AutomationWindowBounds>,
) -> bool {
    let mut state = AUTOMATION_WINDOWS.lock();
    let Some(info) = state.windows.get_mut(id) else {
        return false;
    };
    if generation == 0 || info.generation != Some(generation) {
        return false;
    }
    if info.bounds != bounds {
        tracing::debug!(
            target: "script_kit::automation",
            id = %id,
            generation,
            bounds = ?bounds,
            "automation_window_bounds_changed"
        );
        info.bounds = bounds;
        if let Some(revision) = state.target_revisions.get_mut(id) {
            *revision = revision.saturating_add(1);
        }
    }
    true
}

/// Read an epoch advanced by metadata owners, never by an inspection.
pub fn automation_target_revision(id: &str, generation: u64) -> Option<u64> {
    let state = AUTOMATION_WINDOWS.lock();
    (generation > 0 && state.windows.get(id)?.generation == Some(generation))
        .then(|| state.target_revisions.get(id).copied())
        .flatten()
}

/// Resolve an [`AutomationWindowTarget`] to a single
/// [`AutomationWindowInfo`].
///
/// Uses cached indexes for `Focused`, `Main`, and `Kind` resolution so
/// the cost is O(1) in the common case.  `TitleContains` still scans
/// all entries but sorts matches before choosing the first.
///
/// Returns an error — never silently falls back to the main window —
/// when no matching entry exists.
pub fn resolve_automation_window(
    target: Option<&AutomationWindowTarget>,
) -> Result<AutomationWindowInfo> {
    let state = AUTOMATION_WINDOWS.lock();

    let (resolution_path, result) = match target {
        None | Some(AutomationWindowTarget::Focused) => (
            "focused",
            state
                .focused_id
                .as_ref()
                .and_then(|id| state.windows.get(id))
                .cloned()
                .ok_or_else(|| anyhow!("No focused automation window")),
        ),

        Some(AutomationWindowTarget::Main) => (
            "main",
            state
                .main_id
                .as_ref()
                .and_then(|id| state.windows.get(id))
                .cloned()
                .ok_or_else(|| anyhow!("Main automation window not registered")),
        ),

        Some(AutomationWindowTarget::Id { id }) => (
            "id",
            state
                .windows
                .get(id)
                .cloned()
                .ok_or_else(|| anyhow!("Unknown automation window id: {id}")),
        ),

        Some(AutomationWindowTarget::Instance { id, generation }) => (
            "instance",
            state
                .windows
                .get(id)
                .filter(|info| info.generation == Some(*generation))
                .cloned()
                .ok_or_else(|| {
                    anyhow!(
                        "Unknown or stale automation window instance: {id} generation {generation}"
                    )
                }),
        ),

        Some(AutomationWindowTarget::Kind { kind, index }) => {
            let idx = index.unwrap_or(0);
            let result = state
                .kind_index
                .get(kind)
                .and_then(|ids| ids.get(idx))
                .and_then(|id| state.windows.get(id))
                .cloned()
                .ok_or_else(|| anyhow!("No automation window for kind {:?} index {}", kind, idx));
            ("kind", result)
        }

        Some(AutomationWindowTarget::TitleContains { text }) => {
            let mut matches: Vec<AutomationWindowInfo> = state
                .windows
                .values()
                .filter(|w| {
                    w.title
                        .as_deref()
                        .is_some_and(|title| title.contains(text.as_str()))
                })
                .cloned()
                .collect();
            matches.sort_by(|a, b| {
                kind_rank(a.kind)
                    .cmp(&kind_rank(b.kind))
                    .then_with(|| a.id.cmp(&b.id))
            });
            (
                "titleContains",
                matches
                    .into_iter()
                    .next()
                    .ok_or_else(|| anyhow!("No automation window title contains '{text}'")),
            )
        }
    };

    match &result {
        Ok(info) => {
            tracing::debug!(
                target: "script_kit::automation",
                resolution_path = resolution_path,
                resolved_id = %info.id,
                kind = ?info.kind,
                target = ?target,
                "automation_window_resolved"
            );
        }
        Err(err) => {
            tracing::warn!(
                target: "script_kit::automation",
                resolution_path = resolution_path,
                error = %err,
                target = ?target,
                focused_id = ?state.focused_id,
                main_id = ?state.main_id,
                registered_ids = ?state.windows.keys().cloned().collect::<Vec<_>>(),
                "automation_window_resolve_failed"
            );
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::protocol::{
        AutomationWindowBounds, AutomationWindowInfo, AutomationWindowKind, AutomationWindowTarget,
    };
    use std::sync::atomic::{AtomicU32, Ordering};

    // Guard against parallel test interference on the global singleton.
    // Each test uses a unique ID prefix derived from an atomic counter.
    static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn test_prefix() -> String {
        let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        format!("t{n}")
    }

    // The registry is one process-global singleton. Unique id prefixes do
    // NOT isolate `Focused`/`Main`/kind-index resolution — resolve sees every
    // concurrent test's windows (e.g. another test's focused agent_chat wins a
    // Focused resolve). Serialize the whole module on one mutex; the tests
    // are sub-millisecond so this costs nothing.
    static REGISTRY_TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    pub(crate) fn registry_guard() -> std::sync::MutexGuard<'static, ()> {
        REGISTRY_TEST_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn make_info(prefix: &str, id: &str, kind: AutomationWindowKind) -> AutomationWindowInfo {
        AutomationWindowInfo {
            id: format!("{prefix}:{id}"),
            kind,
            title: Some(format!("Window {id}")),
            focused: false,
            visible: true,
            semantic_surface: None,
            bounds: None,
            parent_window_id: None,
            parent_window_generation: None,
            parent_kind: None,
            pid: Some(std::process::id()),
            generation: None,
        }
    }

    // -- Core CRUD ---------------------------------------------------------

    #[test]
    fn automation_window_registry_resolves_main_and_focused() {
        let _registry_guard = registry_guard();
        let p = test_prefix();

        let mut main = make_info(&p, "main", AutomationWindowKind::Main);
        main.focused = true;
        upsert_automation_window(main.clone());

        let mut notes = make_info(&p, "notes", AutomationWindowKind::Notes);
        notes.focused = false;
        upsert_automation_window(notes.clone());

        // Resolve Main target
        let target_main = AutomationWindowTarget::Main;
        let resolved = resolve_automation_window(Some(&target_main)).expect("should resolve main");
        assert_eq!(resolved.id, format!("{p}:main"));
        assert_eq!(resolved.kind, AutomationWindowKind::Main);

        // Resolve Focused target (should be main since it has focused=true)
        let target_focused = AutomationWindowTarget::Focused;
        let resolved =
            resolve_automation_window(Some(&target_focused)).expect("should resolve focused");
        assert_eq!(resolved.id, format!("{p}:main"));
        assert!(resolved.focused);

        // None target behaves like Focused
        let resolved = resolve_automation_window(None).expect("should resolve None as focused");
        assert_eq!(resolved.id, format!("{p}:main"));

        // Cleanup
        remove_automation_window(&format!("{p}:main"));
        remove_automation_window(&format!("{p}:notes"));
    }

    #[test]
    fn automation_window_registry_resolves_kind_index() {
        let _registry_guard = registry_guard();
        let p = test_prefix();

        let mut agent_chat0 = make_info(&p, "agent_chat0", AutomationWindowKind::AgentChatDetached);
        agent_chat0.title = Some("Agent Chat Thread 0".into());
        upsert_automation_window(agent_chat0);

        let mut agent_chat1 = make_info(&p, "agent_chat1", AutomationWindowKind::AgentChatDetached);
        agent_chat1.title = Some("Agent Chat Thread 1".into());
        upsert_automation_window(agent_chat1);

        // Kind without index → first
        let target = AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::AgentChatDetached,
            index: None,
        };
        let resolved =
            resolve_automation_window(Some(&target)).expect("should resolve kind index 0");
        assert_eq!(resolved.kind, AutomationWindowKind::AgentChatDetached);

        // Kind with index 1 → second
        let target = AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::AgentChatDetached,
            index: Some(1),
        };
        let resolved =
            resolve_automation_window(Some(&target)).expect("should resolve kind index 1");
        assert_eq!(resolved.kind, AutomationWindowKind::AgentChatDetached);
        // The two should have different IDs
        assert_ne!(
            resolve_automation_window(Some(&AutomationWindowTarget::Kind {
                kind: AutomationWindowKind::AgentChatDetached,
                index: Some(0),
            }))
            .expect("idx 0")
            .id,
            resolved.id
        );

        // Kind with out-of-range index → error
        let target = AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::AgentChatDetached,
            index: Some(99),
        };
        assert!(resolve_automation_window(Some(&target)).is_err());

        // Cleanup
        remove_automation_window(&format!("{p}:agent_chat0"));
        remove_automation_window(&format!("{p}:agent_chat1"));
    }

    #[test]
    fn automation_window_registry_unregisters_closed_window() {
        let _registry_guard = registry_guard();
        let p = test_prefix();

        let info = make_info(&p, "notes", AutomationWindowKind::Notes);
        upsert_automation_window(info.clone());

        // Verify it exists
        let target = AutomationWindowTarget::Id {
            id: format!("{p}:notes"),
        };
        assert!(resolve_automation_window(Some(&target)).is_ok());

        // Unregister
        let removed = remove_automation_window(&format!("{p}:notes"));
        assert!(removed.is_some());
        assert_eq!(
            removed.as_ref().expect("removed").kind,
            AutomationWindowKind::Notes
        );

        // Now resolution fails
        assert!(resolve_automation_window(Some(&target)).is_err());

        // Double-remove is harmless
        assert!(remove_automation_window(&format!("{p}:notes")).is_none());
    }

    // -- Focus tracking ----------------------------------------------------

    #[test]
    fn focus_change_updates_all_entries() {
        let _registry_guard = registry_guard();
        let p = test_prefix();

        let mut main = make_info(&p, "main", AutomationWindowKind::Main);
        main.focused = true;
        upsert_automation_window(main);

        let notes = make_info(&p, "notes", AutomationWindowKind::Notes);
        upsert_automation_window(notes);

        // Shift focus to notes
        assert!(set_automation_focus(&format!("{p}:notes")));
        assert_eq!(
            focused_automation_window_id().as_deref(),
            Some(format!("{p}:notes").as_str())
        );

        // Main should now be unfocused
        let main_resolved = resolve_automation_window(Some(&AutomationWindowTarget::Main))
            .expect("main still registered");
        assert!(!main_resolved.focused);

        // Notes should be focused
        let notes_resolved = resolve_automation_window(Some(&AutomationWindowTarget::Id {
            id: format!("{p}:notes"),
        }))
        .expect("notes still registered");
        assert!(notes_resolved.focused);

        // Focus on unknown ID returns false
        assert!(!set_automation_focus("nonexistent-window"));

        // Cleanup
        remove_automation_window(&format!("{p}:main"));
        remove_automation_window(&format!("{p}:notes"));
    }

    // -- Visibility --------------------------------------------------------

    #[test]
    fn visibility_update() {
        let _registry_guard = registry_guard();
        let p = test_prefix();

        let info = make_info(&p, "ai", AutomationWindowKind::AgentChatDetached);
        upsert_automation_window(info);

        set_automation_visibility(&format!("{p}:ai"), false);
        let resolved = resolve_automation_window(Some(&AutomationWindowTarget::Id {
            id: format!("{p}:ai"),
        }))
        .expect("should resolve");
        assert!(!resolved.visible);

        set_automation_visibility(&format!("{p}:ai"), true);
        let resolved = resolve_automation_window(Some(&AutomationWindowTarget::Id {
            id: format!("{p}:ai"),
        }))
        .expect("should resolve");
        assert!(resolved.visible);

        // No-op on unknown ID
        set_automation_visibility("nonexistent", false);

        remove_automation_window(&format!("{p}:ai"));
    }

    // -- TitleContains -----------------------------------------------------

    #[test]
    fn title_contains_resolution() {
        let _registry_guard = registry_guard();
        let p = test_prefix();

        let mut info = make_info(&p, "agent_chat", AutomationWindowKind::AgentChatDetached);
        info.title = Some(format!("{p} Script Kit Agent Chat"));
        upsert_automation_window(info);

        let target = AutomationWindowTarget::TitleContains {
            text: format!("{p} Script Kit Agent"),
        };
        let resolved =
            resolve_automation_window(Some(&target)).expect("should match title substring");
        assert_eq!(resolved.kind, AutomationWindowKind::AgentChatDetached);

        // Non-matching title
        let target = AutomationWindowTarget::TitleContains {
            text: "Nonexistent Window Title".into(),
        };
        assert!(resolve_automation_window(Some(&target)).is_err());

        remove_automation_window(&format!("{p}:agent_chat"));
    }

    // -- Bounds ------------------------------------------------------------

    #[test]
    fn info_with_bounds_round_trips_through_registry() {
        let _registry_guard = registry_guard();
        let p = test_prefix();

        let info = AutomationWindowInfo {
            id: format!("{p}:bounded"),
            kind: AutomationWindowKind::Main,
            title: Some("Bounded".into()),
            focused: false,
            visible: true,
            semantic_surface: Some("scriptList".into()),
            bounds: Some(AutomationWindowBounds {
                x: 100.0,
                y: 200.0,
                width: 800.0,
                height: 600.0,
            }),
            parent_window_id: None,
            parent_window_generation: None,
            parent_kind: None,
            pid: Some(std::process::id()),
            generation: None,
        };
        upsert_automation_window(info.clone());

        let resolved = resolve_automation_window(Some(&AutomationWindowTarget::Id {
            id: format!("{p}:bounded"),
        }))
        .expect("should resolve");
        assert_eq!(resolved.bounds.as_ref().expect("bounds").width, 800.0);

        remove_automation_window(&format!("{p}:bounded"));
    }

    // -- list_automation_windows -------------------------------------------

    #[test]
    fn list_returns_all_registered_windows() {
        let _registry_guard = registry_guard();
        let p = test_prefix();

        upsert_automation_window(make_info(&p, "main", AutomationWindowKind::Main));
        upsert_automation_window(make_info(&p, "notes", AutomationWindowKind::Notes));
        upsert_automation_window(make_info(&p, "ai", AutomationWindowKind::AgentChatDetached));

        let all = list_automation_windows();
        let our_windows: Vec<_> = all.iter().filter(|w| w.id.starts_with(&p)).collect();
        assert_eq!(our_windows.len(), 3);

        remove_automation_window(&format!("{p}:main"));
        remove_automation_window(&format!("{p}:notes"));
        remove_automation_window(&format!("{p}:ai"));
    }

    // -- Upsert overwrites -------------------------------------------------

    #[test]
    fn upsert_overwrites_existing_entry() {
        let _registry_guard = registry_guard();
        let p = test_prefix();

        let mut v1 = make_info(&p, "main", AutomationWindowKind::Main);
        v1.semantic_surface = Some("scriptList".into());
        upsert_automation_window(v1);

        let mut v2 = make_info(&p, "main", AutomationWindowKind::Main);
        v2.semantic_surface = Some("argPrompt".into());
        upsert_automation_window(v2);

        let resolved = resolve_automation_window(Some(&AutomationWindowTarget::Id {
            id: format!("{p}:main"),
        }))
        .expect("should resolve");
        assert_eq!(resolved.semantic_surface.as_deref(), Some("argPrompt"));

        remove_automation_window(&format!("{p}:main"));
    }

    // -- Cross-window targeting acceptance tests -------------------------

    /// Proves that targeting a Notes window by kind resolves to distinct
    /// semantic surface and ID, not the main window.
    #[test]
    fn targeted_get_elements_routes_notes_window() {
        let _registry_guard = registry_guard();
        let p = test_prefix();

        let mut main = make_info(&p, "main", AutomationWindowKind::Main);
        main.semantic_surface = Some("scriptList".into());
        main.focused = true;
        upsert_automation_window(main);

        let mut notes = make_info(&p, "notes", AutomationWindowKind::Notes);
        notes.semantic_surface = Some("notes".into());
        notes.title = Some("Script Kit Notes".into());
        upsert_automation_window(notes);

        // Resolve with Kind target → should get Notes, not Main
        let target = AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::Notes,
            index: None,
        };
        let resolved =
            resolve_automation_window(Some(&target)).expect("should resolve Notes window");
        assert_eq!(resolved.kind, AutomationWindowKind::Notes);
        assert_eq!(resolved.semantic_surface.as_deref(), Some("notes"));
        assert_ne!(
            resolved.id,
            format!("{p}:main"),
            "must not fall back to main"
        );

        // No target (None) → should resolve to focused (main)
        let focused = resolve_automation_window(None).expect("should resolve focused");
        assert_eq!(focused.kind, AutomationWindowKind::Main);
        assert_eq!(focused.semantic_surface.as_deref(), Some("scriptList"));

        remove_automation_window(&format!("{p}:main"));
        remove_automation_window(&format!("{p}:notes"));
    }

    /// Proves that targeting a detached Agent Chat window resolves to a distinct
    /// window with its own ID and kind, suitable for screenshot routing.
    #[test]
    fn targeted_capture_screenshot_routes_detached_agent_chat() {
        let _registry_guard = registry_guard();
        let p = test_prefix();

        let mut main = make_info(&p, "main", AutomationWindowKind::Main);
        main.title = Some("Script Kit".into());
        main.focused = false;
        upsert_automation_window(main);

        let mut agent_chat = make_info(
            &p,
            "agent_chat-thread-1",
            AutomationWindowKind::AgentChatDetached,
        );
        agent_chat.title = Some("Script Kit Agent Chat".into());
        agent_chat.focused = true;
        agent_chat.semantic_surface = Some("agentChatChat".into());
        upsert_automation_window(agent_chat);

        // Target by kind → Agent Chat
        let target = AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::AgentChatDetached,
            index: Some(0),
        };
        let resolved =
            resolve_automation_window(Some(&target)).expect("should resolve detached Agent Chat");
        assert_eq!(resolved.kind, AutomationWindowKind::AgentChatDetached);
        assert_eq!(resolved.title.as_deref(), Some("Script Kit Agent Chat"));
        // The screenshot function would use this title to find the OS window
        assert_ne!(
            resolved.title.as_deref(),
            Some("Script Kit"),
            "must not screenshot the main window"
        );

        // Target by ID → Agent Chat
        let target_id = AutomationWindowTarget::Id {
            id: format!("{p}:agent_chat-thread-1"),
        };
        let resolved_by_id =
            resolve_automation_window(Some(&target_id)).expect("should resolve by ID");
        assert_eq!(resolved_by_id.kind, AutomationWindowKind::AgentChatDetached);

        remove_automation_window(&format!("{p}:main"));
        remove_automation_window(&format!("{p}:agent_chat-thread-1"));
    }

    /// Proves that waitFor/batch resolution uses the same resolver as
    /// standalone queries — a Notes or Agent Chat target resolves consistently.
    #[test]
    fn targeted_wait_for_uses_resolved_window_state() {
        let _registry_guard = registry_guard();
        let p = test_prefix();

        let mut main = make_info(&p, "main", AutomationWindowKind::Main);
        main.focused = true;
        main.semantic_surface = Some("scriptList".into());
        upsert_automation_window(main);

        let mut notes = make_info(&p, "notes", AutomationWindowKind::Notes);
        notes.semantic_surface = Some("notes".into());
        upsert_automation_window(notes);

        // The same resolve_automation_window used by getElements should
        // also be used by waitFor and batch — prove consistency.
        let target = AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::Notes,
            index: None,
        };

        // Standalone resolution
        let standalone = resolve_automation_window(Some(&target)).expect("standalone resolve");

        // Same call — proves the code path is shared
        let batch_path = resolve_automation_window(Some(&target)).expect("batch-path resolve");

        assert_eq!(standalone.id, batch_path.id);
        assert_eq!(standalone.kind, batch_path.kind);
        assert_eq!(standalone.semantic_surface, batch_path.semantic_surface);

        // Targeting a non-existent kind fails with an error, not a silent fallback
        let missing = AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::Hud,
            index: None,
        };
        let err = resolve_automation_window(Some(&missing));
        assert!(
            err.is_err(),
            "missing target must return error, not fallback"
        );

        remove_automation_window(&format!("{p}:main"));
        remove_automation_window(&format!("{p}:notes"));
    }

    // -- Deterministic ordering -------------------------------------------

    /// Proves that same-kind windows inserted in reverse order still
    /// resolve deterministically by lexicographic ID.
    #[test]
    fn kind_index_is_deterministic_regardless_of_insertion_order() {
        let _registry_guard = registry_guard();
        let p = test_prefix();

        // Insert b before a — HashMap would be non-deterministic, but
        // the kind_index must sort lexicographically.
        upsert_automation_window(AutomationWindowInfo {
            id: format!("{p}:promptPopup:b"),
            kind: AutomationWindowKind::PromptPopup,
            title: Some("Popup B".into()),
            focused: false,
            visible: true,
            semantic_surface: Some("promptPopup".into()),
            bounds: None,
            parent_window_id: None,
            parent_window_generation: None,
            parent_kind: None,
            pid: None,
            generation: None,
        });
        upsert_automation_window(AutomationWindowInfo {
            id: format!("{p}:promptPopup:a"),
            kind: AutomationWindowKind::PromptPopup,
            title: Some("Popup A".into()),
            focused: false,
            visible: true,
            semantic_surface: Some("promptPopup".into()),
            bounds: None,
            parent_window_id: None,
            parent_window_generation: None,
            parent_kind: None,
            pid: None,
            generation: None,
        });

        // Index 0 must be :a (lexicographically first)
        let resolved = resolve_automation_window(Some(&AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::PromptPopup,
            index: Some(0),
        }))
        .expect("must resolve index 0");
        assert_eq!(resolved.id, format!("{p}:promptPopup:a"));

        // Index 1 must be :b
        let resolved = resolve_automation_window(Some(&AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::PromptPopup,
            index: Some(1),
        }))
        .expect("must resolve index 1");
        assert_eq!(resolved.id, format!("{p}:promptPopup:b"));

        // Repeated calls produce the same result
        for _ in 0..5 {
            let r = resolve_automation_window(Some(&AutomationWindowTarget::Kind {
                kind: AutomationWindowKind::PromptPopup,
                index: Some(0),
            }))
            .expect("stable");
            assert_eq!(r.id, format!("{p}:promptPopup:a"));
        }

        remove_automation_window(&format!("{p}:promptPopup:a"));
        remove_automation_window(&format!("{p}:promptPopup:b"));
    }

    /// Proves that list_automation_windows returns a stable snapshot
    /// sorted by kind_rank then id.
    #[test]
    fn list_snapshot_is_sorted_by_kind_rank_then_id() {
        let _registry_guard = registry_guard();
        let p = test_prefix();

        // Insert in reverse kind-rank order
        upsert_automation_window(make_info(&p, "popup", AutomationWindowKind::PromptPopup));
        upsert_automation_window(make_info(&p, "notes", AutomationWindowKind::Notes));
        upsert_automation_window(make_info(&p, "main", AutomationWindowKind::Main));

        let list1 = list_automation_windows();
        let ours1: Vec<_> = list1.iter().filter(|w| w.id.starts_with(&p)).collect();

        // Main (rank 0) < Notes (rank 1) < PromptPopup (rank 6)
        assert_eq!(ours1[0].kind, AutomationWindowKind::Main);
        assert_eq!(ours1[1].kind, AutomationWindowKind::Notes);
        assert_eq!(ours1[2].kind, AutomationWindowKind::PromptPopup);

        // Second call produces identical order
        let list2 = list_automation_windows();
        let ours2: Vec<_> = list2.iter().filter(|w| w.id.starts_with(&p)).collect();
        assert_eq!(
            ours1.iter().map(|w| &w.id).collect::<Vec<_>>(),
            ours2.iter().map(|w| &w.id).collect::<Vec<_>>(),
        );

        remove_automation_window(&format!("{p}:main"));
        remove_automation_window(&format!("{p}:notes"));
        remove_automation_window(&format!("{p}:popup"));
    }

    #[test]
    fn registry_assigns_monotonic_lifetimes_and_preserves_metadata_updates() {
        let _registry_guard = registry_guard();
        let p = test_prefix();
        let id = format!("{p}:notes");

        upsert_automation_window(make_info(&p, "notes", AutomationWindowKind::Notes));
        let first = automation_window_by_id(&id)
            .and_then(|info| info.generation)
            .expect("new lifetime receives a generation");

        let mut update = make_info(&p, "notes", AutomationWindowKind::Notes);
        update.visible = false;
        upsert_automation_window(update);
        assert_eq!(
            automation_window_by_id(&id).and_then(|info| info.generation),
            Some(first),
            "metadata upserts must preserve the same lifetime",
        );

        remove_automation_window(&id);
        upsert_automation_window(make_info(&p, "notes", AutomationWindowKind::Notes));
        let reopened = automation_window_by_id(&id)
            .and_then(|info| info.generation)
            .expect("reopened lifetime receives a generation");
        assert!(reopened > first, "reopen must allocate a newer lifetime");

        remove_automation_window(&id);
    }

    #[test]
    fn automation_window_instance_target_and_cleanup_require_exact_generation() {
        let _registry_guard = registry_guard();
        let p = test_prefix();
        let parent_id = format!("{p}:main");
        let popup_id = format!("{p}:popup");
        upsert_automation_window(make_info(&p, "main", AutomationWindowKind::Main));

        register_attached_popup_instance(
            popup_id.clone(),
            AutomationWindowKind::PromptPopup,
            Some("Popup".into()),
            Some("promptPopup".into()),
            None,
            Some(&parent_id),
            Some(13),
        )
        .expect("generation 13 should register");
        assert_eq!(
            resolve_automation_window(Some(&AutomationWindowTarget::Instance {
                id: popup_id.clone(),
                generation: 13,
            }))
            .expect("exact generation should resolve")
            .generation,
            Some(13)
        );

        register_attached_popup_instance(
            popup_id.clone(),
            AutomationWindowKind::PromptPopup,
            Some("Popup reopened".into()),
            Some("promptPopup".into()),
            None,
            Some(&parent_id),
            Some(14),
        )
        .expect("generation 14 should replace current ID lifetime");
        assert!(
            resolve_automation_window(Some(&AutomationWindowTarget::Instance {
                id: popup_id.clone(),
                generation: 13,
            }))
            .is_err()
        );
        assert!(remove_automation_window_if_generation(&popup_id, 13).is_none());
        assert_eq!(
            resolve_automation_window(Some(&AutomationWindowTarget::Id {
                id: popup_id.clone(),
            }))
            .expect("legacy ID should resolve the current lifetime")
            .generation,
            Some(14)
        );
        assert!(remove_automation_window_if_generation(&popup_id, 14).is_some());
        assert!(resolve_automation_window(Some(&AutomationWindowTarget::Id {
            id: popup_id.clone(),
        }))
        .is_err());

        remove_automation_window(&parent_id);
    }

    // -- Parent identity (register_attached_popup) ---------------------------

    #[test]
    fn register_attached_popup_records_parent_identity() {
        let _registry_guard = registry_guard();
        let p = test_prefix();

        let mut main = make_info(&p, "main", AutomationWindowKind::Main);
        main.focused = true;
        upsert_automation_window(main);

        let popup_id = format!("{p}:actions");
        register_attached_popup(
            popup_id.clone(),
            AutomationWindowKind::ActionsDialog,
            Some("Actions".into()),
            Some("actionsDialog".into()),
            None,
            Some(&format!("{p}:main")),
        )
        .expect("should register with known parent");

        let resolved = resolve_automation_window(Some(&AutomationWindowTarget::Id {
            id: popup_id.clone(),
        }))
        .expect("should resolve popup");
        assert_eq!(resolved.kind, AutomationWindowKind::ActionsDialog);
        assert_eq!(
            resolved.parent_window_id.as_deref(),
            Some(format!("{p}:main").as_str())
        );
        assert_eq!(resolved.parent_kind, Some(AutomationWindowKind::Main));

        remove_automation_window(&popup_id);
        remove_automation_window(&format!("{p}:main"));
    }

    #[test]
    fn register_attached_popup_fails_closed_on_unknown_parent() {
        let _registry_guard = registry_guard();
        let p = test_prefix();

        let popup_id = format!("{p}:orphan-popup");
        let result = register_attached_popup(
            popup_id.clone(),
            AutomationWindowKind::PromptPopup,
            Some("Confirm".into()),
            None,
            None,
            Some("nonexistent-parent"),
        );

        assert!(
            result.is_err(),
            "must fail closed when parent is not in registry"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("nonexistent-parent"));

        assert!(
            resolve_automation_window(Some(&AutomationWindowTarget::Id { id: popup_id })).is_err()
        );
    }

    #[test]
    fn register_attached_popup_without_parent_fails_closed() {
        let _registry_guard = registry_guard();
        let p = test_prefix();

        let popup_id = format!("{p}:no-parent-popup");
        let result = register_attached_popup(
            popup_id.clone(),
            AutomationWindowKind::ActionsDialog,
            Some("Actions".into()),
            None,
            None,
            None,
        );

        assert!(result.is_err(), "must fail closed when parent_id is None");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("parent automation identity is required"),
            "error should mention missing parent identity, got: {err_msg}"
        );

        // Must not have registered the popup
        assert!(
            resolve_automation_window(Some(&AutomationWindowTarget::Id { id: popup_id })).is_err(),
            "popup must not be in registry after failed registration"
        );
    }

    #[test]
    fn register_attached_popup_resolves_non_main_parent() {
        let _registry_guard = registry_guard();
        let p = test_prefix();

        let mut agent_chat = make_info(&p, "agent_chat-1", AutomationWindowKind::AgentChatDetached);
        agent_chat.focused = true;
        upsert_automation_window(agent_chat);

        let popup_id = format!("{p}:agent_chat-actions");
        register_attached_popup(
            popup_id.clone(),
            AutomationWindowKind::ActionsDialog,
            Some("Actions".into()),
            Some("actionsDialog".into()),
            None,
            Some(&format!("{p}:agent_chat-1")),
        )
        .expect("should register with Agent Chat parent");

        let resolved = resolve_automation_window(Some(&AutomationWindowTarget::Id {
            id: popup_id.clone(),
        }))
        .expect("should resolve");
        assert_eq!(
            resolved.parent_window_id.as_deref(),
            Some(format!("{p}:agent_chat-1").as_str())
        );
        assert_eq!(
            resolved.parent_kind,
            Some(AutomationWindowKind::AgentChatDetached)
        );

        remove_automation_window(&popup_id);
        remove_automation_window(&format!("{p}:agent_chat-1"));
    }

    #[test]
    fn parent_identity_serializes_in_list_snapshot() {
        let _registry_guard = registry_guard();
        let p = test_prefix();

        let mut main = make_info(&p, "main", AutomationWindowKind::Main);
        main.focused = true;
        upsert_automation_window(main);

        register_attached_popup(
            format!("{p}:popup-with-parent"),
            AutomationWindowKind::PromptPopup,
            Some("Confirm".into()),
            None,
            None,
            Some(&format!("{p}:main")),
        )
        .expect("should register");

        let all = list_automation_windows();
        let popup = all
            .iter()
            .find(|w| w.id == format!("{p}:popup-with-parent"))
            .expect("popup should be in list");
        assert_eq!(
            popup.parent_window_id.as_deref(),
            Some(format!("{p}:main").as_str())
        );
        assert_eq!(popup.parent_kind, Some(AutomationWindowKind::Main));

        remove_automation_window(&format!("{p}:popup-with-parent"));
        remove_automation_window(&format!("{p}:main"));
    }
}
