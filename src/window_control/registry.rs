//! Generation-scoped window registry.
//!
//! The registry is the single owner of live window identity. Observation
//! collection stages rows (and retained AX references) OUTSIDE the registry
//! lock; publication then happens atomically under one write lock:
//!
//! 1. match staged rows to previous entries (provider match key, exact
//!    PID + native window number, or `CFEqual` against the previous AX
//!    element — never title/bounds similarity);
//! 2. reuse or allocate nonces;
//! 3. allocate non-rebinding legacy IDs (a numeric ID bound to one nonce is
//!    never rebound to another nonce for the process lifetime);
//! 4. advance the registry generation ONLY when identity membership changed
//!    (bounds/title/focus/minimized/current-space changes increment the
//!    observation revision instead);
//! 5. swap entries/order/legacy maps and drop old AX refs after unlock.
//!
//! Legacy numeric IDs resolve through the current map only; they are never
//! decoded for PID, window index, or authority.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

use anyhow::{bail, Context, Result};

use super::cache::{CachedAxRef, OwnedCachedWindowRef};
use super::ffi::CFEqual;
use super::identity::{next_generation, next_window_nonce};
use super::types::{
    AppIdentity, Bounds, NativeIdConfidence, NativeWindowId, SearchVisibility, WindowCapabilities,
    WindowHandle, WindowInfo, WindowInfoInit, WindowObservation,
};

/// One staged window row produced by observation collection.
pub(super) struct StagedWindow {
    pub app: String,
    pub title: String,
    pub bounds: Bounds,
    pub pid: i32,
    pub bundle_id: Option<String>,
    pub app_path: Option<PathBuf>,
    pub app_order: usize,
    pub window_index: usize,
    pub is_frontmost_app: bool,
    pub is_focused: bool,
    pub is_main: bool,
    pub is_minimized: bool,
    pub is_on_current_space: bool,
    pub role: Option<String>,
    pub subrole: Option<String>,
    pub native_window_id: Option<u32>,
    pub native_id_confidence: NativeIdConfidence,
    /// Preferred legacy ID: the historical `(pid << 16) | index` value for AX
    /// rows, the synthetic CG-derived value for CG-only rows, or the fixture
    /// id for provider rows.
    pub base_legacy_id: u32,
    pub ax: Option<CachedAxRef>,
    pub capabilities: WindowCapabilities,
    pub search_visibility: SearchVisibility,
    /// Deterministic identity key for provider rows (fixture id).
    pub provider_match_key: Option<u32>,
}

pub(super) struct RegistryEntry {
    pub observation: WindowObservation,
    pub ax_window: Option<CachedAxRef>,
    provider_match_key: Option<u32>,
}

struct RegistryState {
    generation: u64,
    observation_revision: u64,
    entries: HashMap<WindowHandle, RegistryEntry>,
    order: Vec<WindowHandle>,
    legacy_ids: HashMap<u32, WindowHandle>,
    /// Process-lifetime tombstones: legacy ID -> nonce it was ever bound to.
    legacy_id_history: HashMap<u32, u64>,
}

static REGISTRY: std::sync::LazyLock<parking_lot::RwLock<RegistryState>> =
    std::sync::LazyLock::new(|| {
        parking_lot::RwLock::new(RegistryState {
            generation: 1,
            observation_revision: 0,
            entries: HashMap::new(),
            order: Vec::new(),
            legacy_ids: HashMap::new(),
            legacy_id_history: HashMap::new(),
        })
    });

/// Read-only registry snapshot metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegistrySnapshot {
    pub generation: u64,
    pub observation_revision: u64,
    pub window_count: usize,
}

/// Which windows a listing should include.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchScope {
    /// Ordinary listings (Window Switcher, SDK getWindows).
    Ordinary,
    /// Every observed window, including internal-only dialogs/sheets.
    All,
}

pub fn registry_snapshot() -> RegistrySnapshot {
    let state = REGISTRY.read();
    RegistrySnapshot {
        generation: state.generation,
        observation_revision: state.observation_revision,
        window_count: state.order.len(),
    }
}

pub(crate) fn registry_generation() -> u64 {
    REGISTRY.read().generation
}

/// Resolve a legacy numeric window ID against the CURRENT registry.
pub fn resolve_legacy_window_id(id: u32) -> Result<WindowHandle> {
    let state = REGISTRY.read();
    let handle = state
        .legacy_ids
        .get(&id)
        .with_context(|| format!("stale or unknown legacy window id {id}"))?;
    Ok(*handle)
}

/// Resolve a handle to its current observation, rejecting stale generations.
pub fn resolve_handle(handle: WindowHandle) -> Result<WindowObservation> {
    let state = REGISTRY.read();
    if handle.registry_generation != state.generation {
        bail!(
            "stale window handle: generation {} != current {}",
            handle.registry_generation,
            state.generation
        );
    }
    let entry = state
        .entries
        .get(&handle)
        .context("window handle not found in current registry")?;
    Ok(entry.observation.clone())
}

/// Resolve a nonce to its current-generation observation (undo targeting).
pub(crate) fn resolve_nonce(nonce: u64) -> Result<WindowObservation> {
    let state = REGISTRY.read();
    state
        .entries
        .values()
        .find(|entry| entry.observation.handle.nonce == nonce)
        .map(|entry| entry.observation.clone())
        .with_context(|| format!("no current window with nonce {nonce}"))
}

/// Independently retained AX reference for a current-generation handle.
pub(crate) fn retained_window(handle: WindowHandle) -> Result<OwnedCachedWindowRef> {
    let state = REGISTRY.read();
    if handle.registry_generation != state.generation {
        bail!(
            "stale window handle: generation {} != current {}",
            handle.registry_generation,
            state.generation
        );
    }
    let entry = state
        .entries
        .get(&handle)
        .context("window handle not found in current registry")?;
    entry
        .ax_window
        .as_ref()
        .and_then(CachedAxRef::retain_owned)
        .context("window has no AX reference (not actionable)")
}

/// Current-generation `WindowInfo` rows in observation order.
pub(crate) fn window_infos(scope: SearchScope) -> Vec<WindowInfo> {
    let state = REGISTRY.read();
    let mut app_orders: HashMap<i32, usize> = HashMap::new();
    let mut next_app_order = 0usize;
    state
        .order
        .iter()
        .filter_map(|handle| state.entries.get(handle))
        .filter(|entry| match scope {
            SearchScope::All => true,
            SearchScope::Ordinary => {
                entry.observation.search_visibility == SearchVisibility::Ordinary
            }
        })
        .enumerate()
        .map(|(global_order, entry)| {
            let observation = &entry.observation;
            let app_order = *app_orders.entry(observation.handle.pid).or_insert_with(|| {
                let order = next_app_order;
                next_app_order += 1;
                order
            });
            WindowInfo::new(WindowInfoInit {
                id: observation.legacy_id,
                app: observation.app.localized_name.clone(),
                title: observation.title.clone(),
                bounds: observation.bounds,
                pid: observation.handle.pid,
                bundle_id: observation.app.bundle_id.clone(),
                app_path: observation.app.app_path.clone(),
                app_order,
                window_index: observation.legacy_id as usize & 0xffff,
                global_order,
                is_frontmost_app: observation.frontmost_app,
                is_focused: observation.focused,
                is_main: observation.main,
                is_minimized: observation.minimized,
                is_on_current_space: observation.current_space,
                handle: observation.handle,
            })
        })
        .collect()
}

/// Allocate a non-rebinding legacy ID for `nonce`, preferring `base`.
fn allocate_legacy_id(
    base: u32,
    nonce: u64,
    legacy_id_history: &mut HashMap<u32, u64>,
    used_this_refresh: &HashSet<u32>,
) -> Option<u32> {
    for offset in 0..=u32::from(u16::MAX) {
        let candidate = base.wrapping_add(offset);
        if used_this_refresh.contains(&candidate) {
            continue;
        }
        match legacy_id_history.get(&candidate) {
            None => {
                legacy_id_history.insert(candidate, nonce);
                return Some(candidate);
            }
            Some(&bound_nonce) if bound_nonce == nonce => return Some(candidate),
            Some(_) => continue,
        }
    }
    None
}

fn ax_refs_equal(previous: &CachedAxRef, staged: &CachedAxRef) -> bool {
    if previous.ptr_usize() == staged.ptr_usize() {
        return true;
    }
    // SAFETY: both pointers are live retained CF objects owned by CachedAxRef.
    unsafe { CFEqual(previous.ptr_usize() as _, staged.ptr_usize() as _) }
}

/// Atomically publish a staged observation set.
pub(super) fn publish_staged(staged: Vec<StagedWindow>) -> RegistrySnapshot {
    let mut state = REGISTRY.write();

    // Previous identity lookups.
    let mut previous_by_provider_key: HashMap<u32, u64> = HashMap::new();
    let mut previous_by_native: HashMap<(i32, u32), u64> = HashMap::new();
    for entry in state.entries.values() {
        let handle = entry.observation.handle;
        if let Some(key) = entry.provider_match_key {
            previous_by_provider_key.insert(key, handle.nonce);
        }
        if let Some(native) = handle.native_window_id {
            previous_by_native.insert((handle.pid, native.0), handle.nonce);
        }
    }

    // Step 1+2: match staged rows to previous nonces or allocate new ones.
    let mut matched_nonces: Vec<u64> = Vec::with_capacity(staged.len());
    for row in &staged {
        let mut nonce = None;
        if let Some(key) = row.provider_match_key {
            nonce = previous_by_provider_key.get(&key).copied();
        }
        if nonce.is_none() {
            if let Some(native) = row.native_window_id {
                nonce = previous_by_native.get(&(row.pid, native)).copied();
            }
        }
        if nonce.is_none() {
            if let Some(staged_ax) = row.ax.as_ref() {
                nonce = state.entries.values().find_map(|entry| {
                    let same_pid = entry.observation.handle.pid == row.pid;
                    let matches = same_pid
                        && entry
                            .ax_window
                            .as_ref()
                            .is_some_and(|previous| ax_refs_equal(previous, staged_ax));
                    matches.then_some(entry.observation.handle.nonce)
                });
            }
        }
        // Guard against two staged rows claiming one previous nonce.
        if let Some(candidate) = nonce {
            if matched_nonces.contains(&candidate) {
                nonce = None;
            }
        }
        matched_nonces.push(nonce.unwrap_or_else(next_window_nonce));
    }

    // Step 3: allocate legacy IDs without rebinding.
    let mut used_this_refresh: HashSet<u32> = HashSet::new();
    let mut assigned_legacy: Vec<Option<u32>> = Vec::with_capacity(staged.len());
    // Split borrow: history map is updated in place.
    let legacy_id_history = &mut state.legacy_id_history;
    for (row, &nonce) in staged.iter().zip(&matched_nonces) {
        let allocated = allocate_legacy_id(
            row.base_legacy_id,
            nonce,
            legacy_id_history,
            &used_this_refresh,
        );
        if let Some(id) = allocated {
            used_this_refresh.insert(id);
        }
        assigned_legacy.push(allocated);
    }

    // Step 4: decide whether identity membership changed.
    let previous_membership: BTreeSet<(u64, Option<u32>)> = state
        .entries
        .values()
        .map(|entry| {
            (
                entry.observation.handle.nonce,
                Some(entry.observation.legacy_id),
            )
        })
        .collect();
    let new_membership: BTreeSet<(u64, Option<u32>)> = matched_nonces
        .iter()
        .zip(&assigned_legacy)
        .map(|(&nonce, &legacy)| (nonce, legacy))
        .collect();
    let generation = if previous_membership == new_membership {
        state.generation
    } else {
        next_generation(state.generation)
    };

    // Step 5: build and swap.
    let mut entries = HashMap::with_capacity(staged.len());
    let mut order = Vec::with_capacity(staged.len());
    let mut legacy_ids = HashMap::new();
    for ((row, nonce), legacy) in staged.into_iter().zip(matched_nonces).zip(assigned_legacy) {
        let mut capabilities = row.capabilities;
        let legacy_id = match legacy {
            Some(id) => id,
            None => {
                // Exhausted: observable but non-actionable.
                capabilities = WindowCapabilities::non_actionable("legacy_id_exhausted");
                u32::MAX
            }
        };
        let handle = WindowHandle {
            pid: row.pid,
            native_window_id: row.native_window_id.map(NativeWindowId),
            registry_generation: generation,
            nonce,
        };
        let observation = WindowObservation {
            handle,
            legacy_id,
            app: AppIdentity {
                bundle_id: row.bundle_id,
                app_path: row.app_path,
                localized_name: row.app,
            },
            title: row.title,
            role: row.role,
            subrole: row.subrole,
            bounds: row.bounds,
            display_id: None,
            minimized: row.is_minimized,
            focused: row.is_focused,
            main: row.is_main,
            frontmost_app: row.is_frontmost_app,
            current_space: row.is_on_current_space,
            capabilities,
            native_id_confidence: row.native_id_confidence,
            search_visibility: row.search_visibility,
        };
        if legacy.is_some() {
            legacy_ids.insert(legacy_id, handle);
        }
        order.push(handle);
        entries.insert(
            handle,
            RegistryEntry {
                observation,
                ax_window: row.ax,
                provider_match_key: row.provider_match_key,
            },
        );
    }

    let old_entries = std::mem::replace(&mut state.entries, entries);
    state.order = order;
    state.legacy_ids = legacy_ids;
    state.generation = generation;
    state.observation_revision = state.observation_revision.wrapping_add(1);
    let snapshot = RegistrySnapshot {
        generation: state.generation,
        observation_revision: state.observation_revision,
        window_count: state.order.len(),
    };
    drop(state);
    // Old AX refs release outside the lock.
    drop(old_entries);
    snapshot
}

/// Refresh the registry from the deterministic test provider.
pub(super) fn refresh_from_test_provider() -> Result<RegistrySnapshot> {
    let states = super::test_support::provider_states()?;
    let live: Vec<_> = states
        .into_iter()
        .filter(|window| !window.destroyed)
        .collect();

    // Native-tab grouping: rows declaring the same tab group AND sharing one
    // native window id form a proven group. Primary: focused > main > lowest
    // fixture id; non-primary members leave the ordinary listing.
    let mut group_members: HashMap<(String, u32), Vec<u32>> = HashMap::new();
    for window in &live {
        if let (Some(group), Some(native)) = (
            window.definition.native_tab_group.clone(),
            window.definition.native_window_id,
        ) {
            group_members
                .entry((group, native))
                .or_default()
                .push(window.id);
        }
    }
    let mut tab_grouped: HashMap<u32, bool> = HashMap::new(); // id -> is_primary
    for members in group_members.values() {
        if members.len() < 2 {
            continue;
        }
        let Some(primary) = members
            .iter()
            .find(|id| {
                live.iter()
                    .any(|window| window.id == **id && window.focused)
            })
            .or_else(|| {
                members
                    .iter()
                    .find(|id| live.iter().any(|window| window.id == **id && window.main))
            })
            .copied()
            .or_else(|| members.iter().copied().min())
        else {
            continue;
        };
        for id in members {
            tab_grouped.insert(*id, *id == primary);
        }
    }

    let staged = live
        .into_iter()
        .map(|window| {
            let untitled_internal = window.definition.title.is_empty()
                && matches!(
                    window.definition.role.as_deref(),
                    Some("AXDialog") | Some("AXSheet")
                );
            let tab_membership = tab_grouped.get(&window.id).copied();
            StagedWindow {
                app: window.definition.app.clone(),
                title: window.definition.title.clone(),
                bounds: window.bounds,
                pid: window.pid,
                bundle_id: window.definition.bundle_id.clone(),
                app_path: window.definition.app_path.clone(),
                app_order: 0,
                window_index: window.id as usize,
                is_frontmost_app: window.definition.frontmost_app.unwrap_or(false),
                is_focused: window.focused,
                is_main: window.main,
                is_minimized: window.minimized,
                is_on_current_space: window.definition.current_space.unwrap_or(true),
                role: window.definition.role.clone(),
                subrole: window.definition.subrole.clone(),
                native_window_id: window.definition.native_window_id,
                native_id_confidence: if tab_membership.is_some() {
                    NativeIdConfidence::NativeTabGroup
                } else if window.definition.native_window_id.is_some() {
                    NativeIdConfidence::UniquePublicCorrelation
                } else {
                    NativeIdConfidence::Unavailable
                },
                base_legacy_id: window.id,
                ax: None,
                capabilities: WindowCapabilities {
                    can_move: window.can_move(),
                    can_resize: window.can_resize(),
                    can_minimize: window.can_minimize(),
                    can_close: window.can_close(),
                    can_raise: window.can_raise(),
                    can_set_fullscreen: window.can_set_fullscreen(),
                    actionable: true,
                    non_actionable_reason: None,
                },
                search_visibility: if untitled_internal || tab_membership == Some(false) {
                    SearchVisibility::InternalOnly
                } else {
                    SearchVisibility::Ordinary
                },
                provider_match_key: Some(window.id),
            }
        })
        .collect();
    Ok(publish_staged(staged))
}

/// Refresh the registry from live system observation.
pub(super) fn refresh_from_system() -> Result<RegistrySnapshot> {
    let staged = super::query::collect_staged_system_windows()?;
    Ok(publish_staged(staged))
}

/// Refresh from whichever backend is active.
pub fn refresh_window_registry() -> Result<RegistrySnapshot> {
    if super::test_support::is_active() {
        refresh_from_test_provider()
    } else {
        refresh_from_system()
    }
}

/// Match a previously resolved AX element to the registry, or insert it.
///
/// Used by the previous-app targeting path: the focused/main/first AX window
/// of the menu-bar-owning app is resolved directly, then reconciled here so
/// the returned `WindowInfo` always carries a current-generation handle.
pub(super) fn upsert_previous_app_window(
    pid: i32,
    app_name: String,
    title: String,
    bounds: Bounds,
    ax: CachedAxRef,
) -> WindowInfo {
    // Fast path: an existing entry for this PID with an equal AX element.
    {
        let state = REGISTRY.read();
        let matched = state.order.iter().find_map(|handle| {
            let entry = state.entries.get(handle)?;
            if entry.observation.handle.pid != pid {
                return None;
            }
            let previous = entry.ax_window.as_ref()?;
            ax_refs_equal(previous, &ax).then_some(entry.observation.clone())
        });
        if let Some(observation) = matched {
            return WindowInfo::new(WindowInfoInit {
                id: observation.legacy_id,
                app: observation.app.localized_name.clone(),
                title: observation.title.clone(),
                bounds: observation.bounds,
                pid,
                bundle_id: observation.app.bundle_id.clone(),
                app_path: observation.app.app_path.clone(),
                app_order: 0,
                window_index: 0,
                global_order: 0,
                is_frontmost_app: true,
                is_focused: observation.focused,
                is_main: observation.main,
                is_minimized: observation.minimized,
                is_on_current_space: observation.current_space,
                handle: observation.handle,
            });
        }
    }

    // Targeted upsert: append the window as a new entry.
    let mut state = REGISTRY.write();
    let nonce = next_window_nonce();
    let base = (pid as u32) << 16;
    let used: HashSet<u32> = state.legacy_ids.keys().copied().collect();
    let legacy = allocate_legacy_id(base, nonce, &mut state.legacy_id_history, &used);
    let generation = next_generation(state.generation);
    state.generation = generation;
    // Membership changed: rebuild every retained handle under the new
    // generation, preserving the previous observation order.
    let entries = std::mem::take(&mut state.entries);
    let previous_order = std::mem::take(&mut state.order);
    let mut rebuilt = HashMap::with_capacity(entries.len() + 1);
    let mut order: Vec<WindowHandle> = Vec::with_capacity(previous_order.len() + 1);
    let mut legacy_ids = HashMap::with_capacity(state.legacy_ids.len() + 1);
    let order_index: HashMap<WindowHandle, usize> = previous_order
        .iter()
        .enumerate()
        .map(|(index, handle)| (*handle, index))
        .collect();
    let mut drained: Vec<(WindowHandle, RegistryEntry)> = entries.into_iter().collect();
    drained.sort_by_key(|(handle, _)| order_index.get(handle).copied().unwrap_or(usize::MAX));
    for (old_handle, mut entry) in drained {
        let new_handle = WindowHandle {
            registry_generation: generation,
            ..old_handle
        };
        entry.observation.handle = new_handle;
        legacy_ids.insert(entry.observation.legacy_id, new_handle);
        order.push(new_handle);
        rebuilt.insert(new_handle, entry);
    }

    let handle = WindowHandle {
        pid,
        native_window_id: None,
        registry_generation: generation,
        nonce,
    };
    let legacy_id = legacy.unwrap_or(u32::MAX);
    let observation = WindowObservation {
        handle,
        legacy_id,
        app: AppIdentity {
            bundle_id: None,
            app_path: None,
            localized_name: app_name.clone(),
        },
        title: title.clone(),
        role: None,
        subrole: None,
        bounds,
        display_id: None,
        minimized: false,
        focused: true,
        main: true,
        frontmost_app: true,
        current_space: true,
        capabilities: WindowCapabilities {
            can_move: true,
            can_resize: true,
            can_minimize: true,
            can_close: true,
            can_raise: true,
            can_set_fullscreen: false,
            actionable: true,
            non_actionable_reason: None,
        },
        native_id_confidence: NativeIdConfidence::Unavailable,
        search_visibility: SearchVisibility::Ordinary,
    };
    if legacy.is_some() {
        legacy_ids.insert(legacy_id, handle);
    }
    order.push(handle);
    rebuilt.insert(
        handle,
        RegistryEntry {
            observation: observation.clone(),
            ax_window: Some(ax),
            provider_match_key: None,
        },
    );
    state.entries = rebuilt;
    state.order = order;
    state.legacy_ids = legacy_ids;
    state.observation_revision = state.observation_revision.wrapping_add(1);
    drop(state);

    WindowInfo::new(WindowInfoInit {
        id: legacy_id,
        app: app_name,
        title,
        bounds,
        pid,
        bundle_id: None,
        app_path: None,
        app_order: 0,
        window_index: 0,
        global_order: 0,
        is_frontmost_app: true,
        is_focused: true,
        is_main: true,
        is_minimized: false,
        is_on_current_space: true,
        handle,
    })
}

/// Shared lock for every test that mutates the global registry.
#[cfg(test)]
pub(super) static REGISTRY_TEST_LOCK: std::sync::LazyLock<parking_lot::Mutex<()>> =
    std::sync::LazyLock::new(|| parking_lot::Mutex::new(()));

#[cfg(test)]
pub(super) fn reset_registry_for_tests() {
    let mut state = REGISTRY.write();
    state.generation = 1;
    state.observation_revision = 0;
    state.entries.clear();
    state.order.clear();
    state.legacy_ids.clear();
    state.legacy_id_history.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn staged(
        key: u32,
        pid: i32,
        title: &str,
        bounds: Bounds,
        base_legacy_id: u32,
    ) -> StagedWindow {
        StagedWindow {
            app: "Test App".to_string(),
            title: title.to_string(),
            bounds,
            pid,
            bundle_id: None,
            app_path: None,
            app_order: 0,
            window_index: 0,
            is_frontmost_app: false,
            is_focused: false,
            is_main: false,
            is_minimized: false,
            is_on_current_space: true,
            role: Some("AXWindow".to_string()),
            subrole: None,
            native_window_id: None,
            native_id_confidence: NativeIdConfidence::Unavailable,
            base_legacy_id,
            ax: None,
            capabilities: WindowCapabilities {
                can_move: true,
                can_resize: true,
                can_minimize: true,
                can_close: true,
                can_raise: true,
                can_set_fullscreen: false,
                actionable: true,
                non_actionable_reason: None,
            },
            search_visibility: SearchVisibility::Ordinary,
            provider_match_key: Some(key),
        }
    }

    #[test]
    fn unchanged_refresh_retains_generation_nonce_and_legacy_id() {
        let _lock = REGISTRY_TEST_LOCK.lock();
        reset_registry_for_tests();
        let bounds = Bounds::new(0, 0, 800, 600);
        publish_staged(vec![staged(1, 100, "One", bounds, (100 << 16) | 0)]);
        let first = registry_snapshot();
        let first_handle = resolve_legacy_window_id((100 << 16) | 0).expect("resolve");

        publish_staged(vec![staged(1, 100, "One", bounds, (100 << 16) | 0)]);
        let second = registry_snapshot();
        let second_handle = resolve_legacy_window_id((100 << 16) | 0).expect("resolve");

        assert_eq!(first.generation, second.generation);
        assert_eq!(first_handle.nonce, second_handle.nonce);
        assert_eq!(first_handle, second_handle);
        assert!(second.observation_revision > first.observation_revision);
    }

    #[test]
    fn bounds_and_title_changes_increment_revision_only() {
        let _lock = REGISTRY_TEST_LOCK.lock();
        reset_registry_for_tests();
        publish_staged(vec![staged(
            1,
            100,
            "One",
            Bounds::new(0, 0, 800, 600),
            (100 << 16) | 0,
        )]);
        let before = registry_snapshot();
        publish_staged(vec![staged(
            1,
            100,
            "Renamed",
            Bounds::new(50, 50, 640, 480),
            (100 << 16) | 0,
        )]);
        let after = registry_snapshot();
        assert_eq!(before.generation, after.generation);
        assert!(after.observation_revision > before.observation_revision);
        let observation =
            resolve_handle(resolve_legacy_window_id((100 << 16) | 0).unwrap()).unwrap();
        assert_eq!(observation.title, "Renamed");
        assert_eq!(observation.bounds, Bounds::new(50, 50, 640, 480));
    }

    #[test]
    fn membership_change_advances_generation_and_stales_old_handles() {
        let _lock = REGISTRY_TEST_LOCK.lock();
        reset_registry_for_tests();
        let bounds = Bounds::new(0, 0, 800, 600);
        publish_staged(vec![staged(1, 100, "One", bounds, (100 << 16) | 0)]);
        let old_handle = resolve_legacy_window_id((100 << 16) | 0).expect("resolve");

        publish_staged(vec![
            staged(1, 100, "One", bounds, (100 << 16) | 0),
            staged(2, 100, "Two", bounds, (100 << 16) | 1),
        ]);
        let error = resolve_handle(old_handle).expect_err("stale generation must fail");
        assert!(error.to_string().contains("stale window handle"));

        // The SAME window resolves under the new generation with its nonce kept.
        let refreshed = resolve_legacy_window_id((100 << 16) | 0).expect("resolve");
        assert_eq!(refreshed.nonce, old_handle.nonce);
        assert!(resolve_handle(refreshed).is_ok());
    }

    #[test]
    fn destroyed_window_makes_its_legacy_id_stale() {
        let _lock = REGISTRY_TEST_LOCK.lock();
        reset_registry_for_tests();
        let bounds = Bounds::new(0, 0, 800, 600);
        publish_staged(vec![
            staged(1, 100, "One", bounds, (100 << 16) | 0),
            staged(2, 100, "Two", bounds, (100 << 16) | 1),
        ]);
        publish_staged(vec![staged(2, 100, "Two", bounds, (100 << 16) | 1)]);
        assert!(resolve_legacy_window_id((100 << 16) | 0).is_err());
        assert!(resolve_legacy_window_id((100 << 16) | 1).is_ok());
    }

    #[test]
    fn recreated_window_at_same_base_receives_a_different_legacy_id() {
        let _lock = REGISTRY_TEST_LOCK.lock();
        reset_registry_for_tests();
        let bounds = Bounds::new(0, 0, 800, 600);
        let base = (100 << 16) | 0;
        publish_staged(vec![staged(1, 100, "Original", bounds, base)]);
        let original_id = {
            let infos = window_infos(SearchScope::Ordinary);
            infos[0].id
        };
        assert_eq!(original_id, base);

        // Window 1 is destroyed; a NEW window (different identity) appears at
        // the same historical base ID.
        publish_staged(vec![staged(2, 100, "Replacement", bounds, base)]);
        let replacement_id = {
            let infos = window_infos(SearchScope::Ordinary);
            infos[0].id
        };
        assert_ne!(
            replacement_id, base,
            "a legacy ID must never be rebound to a different window"
        );
        // The old ID no longer resolves: it belonged to the destroyed window.
        assert!(resolve_legacy_window_id(base).is_err());
        assert!(resolve_legacy_window_id(replacement_id).is_ok());
    }

    #[test]
    fn legacy_ids_resolve_only_within_the_current_registry() {
        let _lock = REGISTRY_TEST_LOCK.lock();
        reset_registry_for_tests();
        let bounds = Bounds::new(0, 0, 800, 600);
        publish_staged(vec![staged(1, 100, "One", bounds, 42)]);
        let handle = resolve_legacy_window_id(42).expect("resolve");
        // PID comes from the observation, never decoded from the ID (42 >> 16 == 0).
        assert_eq!(handle.pid, 100);
        let observation = resolve_handle(handle).expect("observation");
        assert_eq!(observation.handle.pid, 100);
    }

    #[test]
    fn window_infos_exclude_internal_only_rows_from_ordinary_scope() {
        let _lock = REGISTRY_TEST_LOCK.lock();
        reset_registry_for_tests();
        let bounds = Bounds::new(0, 0, 800, 600);
        let mut dialog = staged(2, 100, "", bounds, (100 << 16) | 1);
        dialog.role = Some("AXDialog".to_string());
        dialog.search_visibility = SearchVisibility::InternalOnly;
        publish_staged(vec![staged(1, 100, "One", bounds, (100 << 16) | 0), dialog]);
        assert_eq!(window_infos(SearchScope::Ordinary).len(), 1);
        assert_eq!(window_infos(SearchScope::All).len(), 2);
    }

    #[test]
    fn provider_tab_group_produces_one_ordinary_primary_and_internal_members() {
        let _lock = REGISTRY_TEST_LOCK.lock();
        let _env = super::super::test_support::test_env::EnvGuard::set(
            r#"{"windows":[
                {"id":107,"app":"Tabbed","title":"Tab One","pid":5003,
                 "nativeWindowId":9107,"nativeTabGroup":"group-a","focused":true},
                {"id":108,"app":"Tabbed","title":"Tab Two","pid":5003,
                 "nativeWindowId":9107,"nativeTabGroup":"group-a"}
            ]}"#,
        );
        reset_registry_for_tests();
        refresh_from_test_provider().expect("refresh");

        let ordinary = window_infos(SearchScope::Ordinary);
        assert_eq!(ordinary.len(), 1, "one movable row per native-tab group");
        assert_eq!(ordinary[0].id, 107, "focused member is the primary");
        assert_eq!(window_infos(SearchScope::All).len(), 2);

        let member_handle = resolve_legacy_window_id(108).expect("member resolves");
        let member = resolve_handle(member_handle).expect("member observation");
        assert_eq!(
            member.native_id_confidence,
            NativeIdConfidence::NativeTabGroup
        );
        assert_eq!(member.search_visibility, SearchVisibility::InternalOnly);
    }

    #[test]
    fn provider_untitled_dialog_is_observable_but_not_ordinary() {
        let _lock = REGISTRY_TEST_LOCK.lock();
        let _env = super::super::test_support::test_env::EnvGuard::set(
            r#"{"windows":[
                {"id":1,"app":"A","title":"Doc","pid":9},
                {"id":2,"app":"A","title":"","pid":9,"role":"AXDialog"}
            ]}"#,
        );
        reset_registry_for_tests();
        refresh_from_test_provider().expect("refresh");
        assert_eq!(window_infos(SearchScope::Ordinary).len(), 1);
        assert_eq!(window_infos(SearchScope::All).len(), 2);
        let dialog = resolve_handle(resolve_legacy_window_id(2).expect("resolve")).expect("obs");
        assert_eq!(dialog.search_visibility, SearchVisibility::InternalOnly);
    }

    #[test]
    fn provider_capability_flags_flow_into_observations() {
        let _lock = REGISTRY_TEST_LOCK.lock();
        let _env = super::super::test_support::test_env::EnvGuard::set(
            r#"{"windows":[
                {"id":1,"app":"A","title":"Frozen","pid":9,
                 "positionSettable":false,"sizeSettable":false}
            ]}"#,
        );
        reset_registry_for_tests();
        refresh_from_test_provider().expect("refresh");
        let observation =
            resolve_handle(resolve_legacy_window_id(1).expect("resolve")).expect("obs");
        assert!(!observation.capabilities.can_move);
        assert!(!observation.capabilities.can_resize);
        assert!(observation.capabilities.can_minimize);
    }

    #[test]
    fn window_info_exposes_no_raw_ax_pointer() {
        // Compile-time-ish check: WindowInfo's debug output must not include
        // an ax pointer field, and the struct offers no pointer accessor.
        let _lock = REGISTRY_TEST_LOCK.lock();
        reset_registry_for_tests();
        publish_staged(vec![staged(
            1,
            100,
            "One",
            Bounds::new(0, 0, 800, 600),
            (100 << 16) | 0,
        )]);
        let infos = window_infos(SearchScope::Ordinary);
        let debug = format!("{:?}", infos[0]);
        assert!(!debug.contains("ax_window"));
    }
}
