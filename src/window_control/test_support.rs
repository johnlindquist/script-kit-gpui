//! Deterministic window-provider fixture support.
//!
//! The `SCRIPT_KIT_WINDOW_SEARCH_TEST_PROVIDER` environment variable supplies a
//! JSON fixture that replaces live AX/CoreGraphics enumeration. Two shapes are
//! accepted:
//!
//! - the historical bare array of windows (`[{"app": "...", "title": "..."}]`),
//!   preserved byte-for-byte in behavior for existing fixtures; and
//! - the v1 document (`{"windows": [...], "displays": [...],
//!   "frontmostWindowId": ...}`) that adds capability flags, native IDs,
//!   native-tab groups, displays, and scripted mutation behavior.
//!
//! Provider state is cached by the EXACT raw environment string: mutations
//! performed against the provider survive later reads of the same fixture, and
//! the state resets only when the raw string changes or disappears. This is
//! what lets transaction tests observe their own effects deterministically.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use super::types::{Bounds, WindowInfo, WindowInfoInit};

const ENV_VAR: &str = "SCRIPT_KIT_WINDOW_SEARCH_TEST_PROVIDER";

fn default_scale_factor() -> f64 {
    2.0
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TestProviderBounds {
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

impl TestProviderBounds {
    fn resolve(&self) -> Bounds {
        Bounds::new(
            self.x.unwrap_or(0),
            self.y.unwrap_or(0),
            self.width.unwrap_or(1280),
            self.height.unwrap_or(720),
        )
    }
}

/// Scripted mutation behavior for one fixture window.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TestProviderMutationBehavior {
    /// Sleep this long (chunked, cancellation-aware) before applying a mutation.
    #[serde(default)]
    pub delay_ms: u64,
    /// Fail the Nth mutation attempt (1-based) with a setter error.
    #[serde(default)]
    pub fail_on_attempt: Option<u8>,
    /// Destroy the window on the Nth mutation attempt (1-based).
    #[serde(default)]
    pub destroy_on_attempt: Option<u8>,
    /// Apply this offset to every requested x (readback mismatch simulation).
    #[serde(default)]
    pub position_delta_x: i32,
    /// Apply this offset to every requested y.
    #[serde(default)]
    pub position_delta_y: i32,
    /// Clamp requested width to at least this value.
    #[serde(default)]
    pub min_width: Option<u32>,
    /// Clamp requested height to at least this value.
    #[serde(default)]
    pub min_height: Option<u32>,
    /// Clamp requested width to at most this value.
    #[serde(default)]
    pub max_width: Option<u32>,
    /// Clamp requested height to at most this value.
    #[serde(default)]
    pub max_height: Option<u32>,
    /// Simulate a close that does not remove the window (save prompt).
    #[serde(default)]
    pub close_leaves_window: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TestProviderWindow {
    #[serde(default)]
    pub id: Option<u32>,
    pub app: String,
    pub title: String,
    #[serde(default)]
    pub pid: Option<i32>,
    #[serde(default)]
    pub bundle_id: Option<String>,
    #[serde(default)]
    pub app_path: Option<PathBuf>,
    #[serde(default)]
    pub native_window_id: Option<u32>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub subrole: Option<String>,
    #[serde(default)]
    pub bounds: Option<TestProviderBounds>,
    #[serde(default)]
    pub display_id: Option<u32>,
    #[serde(default)]
    pub minimized: Option<bool>,
    #[serde(default)]
    pub current_space: Option<bool>,
    #[serde(default)]
    pub focused: Option<bool>,
    #[serde(default)]
    pub main: Option<bool>,
    #[serde(default)]
    pub frontmost_app: Option<bool>,
    #[serde(default)]
    pub position_settable: Option<bool>,
    #[serde(default)]
    pub size_settable: Option<bool>,
    #[serde(default)]
    pub minimized_settable: Option<bool>,
    #[serde(default)]
    pub fullscreen_settable: Option<bool>,
    #[serde(default)]
    pub raise_supported: Option<bool>,
    #[serde(default)]
    pub close_supported: Option<bool>,
    #[serde(default)]
    pub native_tab_group: Option<String>,
    #[serde(default)]
    pub mutation: TestProviderMutationBehavior,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TestProviderDisplay {
    pub id: u32,
    pub uuid: String,
    pub name: String,
    pub full_bounds: TestProviderBounds,
    pub visible_bounds: TestProviderBounds,
    #[serde(default = "default_scale_factor")]
    pub scale_factor: f64,
    #[serde(default)]
    pub is_primary: bool,
    #[serde(default)]
    pub legacy_order: usize,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestProviderDocument {
    #[serde(default)]
    windows: Vec<TestProviderWindow>,
    #[serde(default)]
    displays: Vec<TestProviderDisplay>,
    #[serde(default)]
    frontmost_window_id: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum TestProviderInput {
    LegacyWindows(Vec<TestProviderWindow>),
    Document(TestProviderDocument),
}

/// Live, mutable state for one fixture window.
#[derive(Debug, Clone)]
pub(crate) struct ProviderWindowState {
    pub definition: TestProviderWindow,
    pub id: u32,
    pub pid: i32,
    pub bounds: Bounds,
    pub minimized: bool,
    pub focused: bool,
    pub main: bool,
    pub destroyed: bool,
    pub mutation_attempts: u8,
}

impl ProviderWindowState {
    fn from_definition(index: usize, definition: TestProviderWindow) -> Self {
        let id = definition.id.unwrap_or(index as u32 + 1);
        let pid = definition.pid.unwrap_or(0);
        let bounds = definition
            .bounds
            .as_ref()
            .map(TestProviderBounds::resolve)
            .unwrap_or_else(|| Bounds::new(0, 0, 1280, 720));
        let minimized = definition.minimized.unwrap_or(false);
        let focused = definition.focused.unwrap_or(false);
        let main = definition.main.unwrap_or(false);
        Self {
            definition,
            id,
            pid,
            bounds,
            minimized,
            focused,
            main,
            destroyed: false,
            mutation_attempts: 0,
        }
    }

    pub(crate) fn can_move(&self) -> bool {
        self.definition.position_settable.unwrap_or(true)
    }

    pub(crate) fn can_resize(&self) -> bool {
        self.definition.size_settable.unwrap_or(true)
    }

    pub(crate) fn can_minimize(&self) -> bool {
        self.definition.minimized_settable.unwrap_or(true)
    }

    pub(crate) fn can_set_fullscreen(&self) -> bool {
        self.definition.fullscreen_settable.unwrap_or(false)
    }

    pub(crate) fn can_raise(&self) -> bool {
        self.definition.raise_supported.unwrap_or(true)
    }

    pub(crate) fn can_close(&self) -> bool {
        self.definition.close_supported.unwrap_or(true)
    }
}

#[derive(Debug, Default)]
struct ProviderState {
    raw: String,
    windows: Vec<ProviderWindowState>,
    displays: Vec<TestProviderDisplay>,
    frontmost_window_id: Option<u32>,
    /// Total mutations applied against this fixture instance. Plan-compilation
    /// tests assert this stays zero across planning.
    mutation_count: u64,
}

static PROVIDER: std::sync::LazyLock<parking_lot::Mutex<Option<ProviderState>>> =
    std::sync::LazyLock::new(|| parking_lot::Mutex::new(None));

fn parse_document(raw: &str) -> Result<ProviderState> {
    let input: TestProviderInput =
        serde_json::from_str(raw).with_context(|| format!("Failed to parse {ENV_VAR}"))?;
    let (windows, displays, frontmost_window_id) = match input {
        TestProviderInput::LegacyWindows(windows) => (windows, Vec::new(), None),
        TestProviderInput::Document(document) => (
            document.windows,
            document.displays,
            document.frontmost_window_id,
        ),
    };
    let windows = windows
        .into_iter()
        .enumerate()
        .map(|(index, definition)| ProviderWindowState::from_definition(index, definition))
        .collect::<Vec<_>>();

    let mut seen = HashMap::new();
    for window in &windows {
        if let Some(previous) = seen.insert(window.id, window.definition.title.clone()) {
            bail!(
                "duplicate provider window id {} (\"{previous}\" and \"{}\")",
                window.id,
                window.definition.title
            );
        }
    }

    Ok(ProviderState {
        raw: raw.to_string(),
        windows,
        displays,
        frontmost_window_id,
        mutation_count: 0,
    })
}

/// Ensure provider state matches the current environment value.
///
/// Returns `false` when the provider env var is absent (live mode).
fn sync_provider_state() -> Result<bool> {
    let raw = std::env::var(ENV_VAR)
        .ok()
        .filter(|value| !value.trim().is_empty());
    let mut guard = PROVIDER.lock();
    match raw {
        None => {
            *guard = None;
            Ok(false)
        }
        Some(raw) => {
            let needs_reparse = guard.as_ref().is_none_or(|state| state.raw != raw);
            if needs_reparse {
                *guard = Some(parse_document(&raw)?);
            }
            Ok(true)
        }
    }
}

/// True when the deterministic provider is active for this process.
pub(crate) fn is_active() -> bool {
    sync_provider_state().unwrap_or(false)
}

/// Access the provider state mutably. Fails when the provider is inactive.
pub(crate) fn with_state<T>(f: impl FnOnce(&mut ProviderState) -> T) -> Result<T> {
    if !sync_provider_state()? {
        bail!("test provider is not active");
    }
    let mut guard = PROVIDER.lock();
    let state = guard
        .as_mut()
        .context("test provider state missing after sync")?;
    Ok(f(state))
}

/// Windows in the ordinary listing shape (parity with the legacy provider).
pub(crate) fn provider_window_infos() -> Result<Option<Vec<WindowInfo>>> {
    if !sync_provider_state()? {
        return Ok(None);
    }
    let guard = PROVIDER.lock();
    let Some(state) = guard.as_ref() else {
        return Ok(None);
    };
    let infos = state
        .windows
        .iter()
        .filter(|window| !window.destroyed)
        .enumerate()
        .map(|(order, window)| {
            WindowInfo::new(WindowInfoInit {
                id: window.id,
                app: window.definition.app.clone(),
                title: window.definition.title.clone(),
                bounds: window.bounds,
                pid: window.pid,
                bundle_id: window.definition.bundle_id.clone(),
                app_path: window.definition.app_path.clone(),
                app_order: 0,
                window_index: window.id as usize,
                global_order: order,
                is_frontmost_app: window.definition.frontmost_app.unwrap_or(false),
                is_focused: window.focused,
                is_main: window.main,
                is_minimized: window.minimized,
                is_on_current_space: window.definition.current_space.unwrap_or(true),
                handle: super::types::WindowHandle {
                    pid: window.pid,
                    native_window_id: window
                        .definition
                        .native_window_id
                        .map(super::types::NativeWindowId),
                    registry_generation: 0,
                    nonce: 0,
                },
            })
        })
        .collect();
    Ok(Some(infos))
}

/// Snapshot of every live provider window (registry staging input).
pub(crate) fn provider_states() -> Result<Vec<ProviderWindowState>> {
    with_state(|state| state.windows.clone())
}

/// Snapshot of one provider window used by observation and verification.
pub(crate) fn window_state(id: u32) -> Result<ProviderWindowState> {
    with_state(|state| {
        state
            .windows
            .iter()
            .find(|window| window.id == id && !window.destroyed)
            .cloned()
            .with_context(|| format!("provider window {id} not found"))
    })?
}

/// The provider displays, if the fixture declares any.
pub(crate) fn provider_displays() -> Result<Vec<TestProviderDisplay>> {
    with_state(|state| state.displays.clone())
}

/// The fixture-declared frontmost window, if any.
pub(crate) fn frontmost_window_id() -> Result<Option<u32>> {
    with_state(|state| state.frontmost_window_id)
}

/// Total mutations applied against the current fixture instance.
pub(crate) fn mutation_count() -> Result<u64> {
    with_state(|state| state.mutation_count)
}

/// Outcome of a scripted provider mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProviderMutationOutcome {
    Applied,
    SetterError(String),
    Destroyed,
    Cancelled,
}

fn run_delay(delay_ms: u64, cancelled: Option<&Arc<AtomicBool>>) -> bool {
    let mut remaining = delay_ms;
    while remaining > 0 {
        if cancelled.is_some_and(|flag| flag.load(Ordering::SeqCst)) {
            return false;
        }
        let chunk = remaining.min(10);
        std::thread::sleep(Duration::from_millis(chunk));
        remaining -= chunk;
    }
    !cancelled.is_some_and(|flag| flag.load(Ordering::SeqCst))
}

/// One scripted mutation against a provider window.
///
/// `mutate` receives the window state and applies the requested change,
/// honoring clamps/deltas itself where relevant.
pub(crate) fn apply_mutation(
    id: u32,
    cancelled: Option<&Arc<AtomicBool>>,
    mutate: impl FnOnce(&mut ProviderWindowState),
) -> Result<ProviderMutationOutcome> {
    // Read delay outside the lock so slow windows do not block the provider.
    let delay_ms = with_state(|state| {
        state
            .windows
            .iter()
            .find(|window| window.id == id && !window.destroyed)
            .map(|window| window.definition.mutation.delay_ms)
    })?
    .with_context(|| format!("provider window {id} not found"))?;

    if !run_delay(delay_ms, cancelled) {
        return Ok(ProviderMutationOutcome::Cancelled);
    }

    with_state(|state| {
        let Some(window) = state
            .windows
            .iter_mut()
            .find(|window| window.id == id && !window.destroyed)
        else {
            return ProviderMutationOutcome::SetterError(format!(
                "provider window {id} disappeared"
            ));
        };
        window.mutation_attempts = window.mutation_attempts.saturating_add(1);
        let attempt = window.mutation_attempts;
        state.mutation_count += 1;

        if window
            .definition
            .mutation
            .destroy_on_attempt
            .is_some_and(|destroy_at| attempt >= destroy_at)
        {
            window.destroyed = true;
            return ProviderMutationOutcome::Destroyed;
        }
        if window
            .definition
            .mutation
            .fail_on_attempt
            .is_some_and(|fail_at| attempt == fail_at)
        {
            return ProviderMutationOutcome::SetterError(format!(
                "scripted setter failure on attempt {attempt}"
            ));
        }

        mutate(window);
        ProviderMutationOutcome::Applied
    })
}

/// Apply the fixture's clamp/delta rules to a requested bounds change.
pub(crate) fn resolve_requested_bounds(
    behavior: &TestProviderMutationBehavior,
    requested: Bounds,
) -> Bounds {
    let mut width = requested.width;
    let mut height = requested.height;
    if let Some(min_width) = behavior.min_width {
        width = width.max(min_width);
    }
    if let Some(min_height) = behavior.min_height {
        height = height.max(min_height);
    }
    if let Some(max_width) = behavior.max_width {
        width = width.min(max_width);
    }
    if let Some(max_height) = behavior.max_height {
        height = height.min(max_height);
    }
    Bounds::new(
        requested.x + behavior.position_delta_x,
        requested.y + behavior.position_delta_y,
        width,
        height,
    )
}

#[cfg(test)]
pub(crate) fn reset_for_tests() {
    *PROVIDER.lock() = None;
}

/// Shared provider-env guard for every test module that mutates
/// `SCRIPT_KIT_WINDOW_SEARCH_TEST_PROVIDER`. One process-wide lock prevents
/// parallel test threads from interleaving env mutations.
#[cfg(test)]
pub(crate) mod test_env {
    use super::{reset_for_tests, ENV_VAR};

    static ENV_LOCK: std::sync::LazyLock<parking_lot::Mutex<()>> =
        std::sync::LazyLock::new(|| parking_lot::Mutex::new(()));

    pub(crate) struct EnvGuard {
        _lock: parking_lot::MutexGuard<'static, ()>,
    }

    impl EnvGuard {
        pub(crate) fn set(value: &str) -> Self {
            let lock = ENV_LOCK.lock();
            reset_for_tests();
            std::env::set_var(ENV_VAR, value);
            Self { _lock: lock }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            std::env::remove_var(ENV_VAR);
            reset_for_tests();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_env::EnvGuard;
    use super::*;

    #[test]
    fn legacy_array_input_parses_with_historical_defaults() {
        let _guard = EnvGuard::set(r#"[{"app":"TestApp","title":"One"}]"#);
        let windows = provider_window_infos()
            .expect("provider parse")
            .expect("provider active");
        assert_eq!(windows.len(), 1);
        let window = &windows[0];
        assert_eq!(window.id, 1);
        assert_eq!(window.pid, 0);
        assert_eq!(window.bounds, Bounds::new(0, 0, 1280, 720));
        assert!(window.is_on_current_space);
        assert!(!window.is_focused);
        assert!(!window.is_minimized);
    }

    #[test]
    fn v1_document_parses_windows_displays_and_frontmost() {
        let _guard = EnvGuard::set(
            r#"{
                "windows": [
                    {"id": 7, "app": "A", "title": "T", "pid": 42,
                     "bounds": {"x": 5, "y": 6, "width": 700, "height": 500},
                     "nativeWindowId": 900, "focused": true,
                     "positionSettable": false}
                ],
                "displays": [
                    {"id": 1, "uuid": "uuid-1", "name": "Main",
                     "fullBounds": {"x": 0, "y": 0, "width": 1920, "height": 1080},
                     "visibleBounds": {"x": 0, "y": 25, "width": 1920, "height": 1055},
                     "isPrimary": true}
                ],
                "frontmostWindowId": 7
            }"#,
        );
        let windows = provider_window_infos()
            .expect("provider parse")
            .expect("provider active");
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].id, 7);
        assert_eq!(windows[0].pid, 42);
        assert!(windows[0].is_focused);
        let state = window_state(7).expect("state");
        assert!(!state.can_move());
        assert!(state.can_resize());
        let displays = provider_displays().expect("displays");
        assert_eq!(displays.len(), 1);
        assert_eq!(displays[0].uuid, "uuid-1");
        assert_eq!(frontmost_window_id().expect("frontmost"), Some(7));
    }

    #[test]
    fn missing_optional_fields_retain_historical_defaults() {
        let _guard = EnvGuard::set(r#"{"windows":[{"app":"A","title":"T"}]}"#);
        let state = window_state(1).expect("state");
        assert!(state.can_move());
        assert!(state.can_resize());
        assert!(state.can_minimize());
        assert!(state.can_raise());
        assert!(state.can_close());
        assert!(!state.can_set_fullscreen());
        assert!(!state.focused);
        assert!(!state.minimized);
        assert_eq!(state.bounds, Bounds::new(0, 0, 1280, 720));
    }

    #[test]
    fn mutations_persist_across_reads_of_the_same_raw_fixture() {
        let _guard = EnvGuard::set(r#"{"windows":[{"app":"A","title":"T"}]}"#);
        let outcome = apply_mutation(1, None, |window| {
            window.bounds = Bounds::new(10, 20, 640, 480);
        })
        .expect("mutation");
        assert_eq!(outcome, ProviderMutationOutcome::Applied);

        // A second read of the SAME raw fixture must see the mutation.
        let windows = provider_window_infos()
            .expect("provider parse")
            .expect("provider active");
        assert_eq!(windows[0].bounds, Bounds::new(10, 20, 640, 480));
        assert_eq!(mutation_count().expect("count"), 1);
    }

    #[test]
    fn changing_the_raw_fixture_resets_state() {
        let _guard = EnvGuard::set(r#"{"windows":[{"app":"A","title":"T"}]}"#);
        apply_mutation(1, None, |window| {
            window.bounds = Bounds::new(10, 20, 640, 480);
        })
        .expect("mutation");

        std::env::set_var(ENV_VAR, r#"{"windows":[{"app":"A","title":"T","pid":9}]}"#);
        let windows = provider_window_infos()
            .expect("provider parse")
            .expect("provider active");
        assert_eq!(windows[0].bounds, Bounds::new(0, 0, 1280, 720));
        assert_eq!(windows[0].pid, 9);
        assert_eq!(mutation_count().expect("count"), 0);
    }

    #[test]
    fn scripted_failure_and_destroy_behaviors_fire_by_attempt() {
        let _guard = EnvGuard::set(
            r#"{"windows":[
                {"id":1,"app":"A","title":"Fails",
                 "mutation":{"failOnAttempt":1}},
                {"id":2,"app":"A","title":"Dies",
                 "mutation":{"destroyOnAttempt":2}}
            ]}"#,
        );
        let outcome = apply_mutation(1, None, |_| {}).expect("mutation");
        assert!(matches!(outcome, ProviderMutationOutcome::SetterError(_)));

        assert_eq!(
            apply_mutation(2, None, |_| {}).expect("mutation"),
            ProviderMutationOutcome::Applied
        );
        assert_eq!(
            apply_mutation(2, None, |_| {}).expect("mutation"),
            ProviderMutationOutcome::Destroyed
        );
        // Destroyed windows disappear from listings.
        let windows = provider_window_infos()
            .expect("provider parse")
            .expect("provider active");
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].id, 1);
    }

    #[test]
    fn cancellation_before_delay_completion_prevents_mutation() {
        let _guard = EnvGuard::set(
            r#"{"windows":[
                {"id":1,"app":"A","title":"Slow","mutation":{"delayMs":200}}
            ]}"#,
        );
        let cancelled = Arc::new(AtomicBool::new(true));
        let outcome = apply_mutation(1, Some(&cancelled), |window| {
            window.bounds = Bounds::new(1, 1, 100, 100);
        })
        .expect("mutation");
        assert_eq!(outcome, ProviderMutationOutcome::Cancelled);
        let state = window_state(1).expect("state");
        assert_eq!(state.bounds, Bounds::new(0, 0, 1280, 720));
        assert_eq!(mutation_count().expect("count"), 0);
    }

    #[test]
    fn clamp_and_delta_rules_resolve_requested_bounds() {
        let behavior = TestProviderMutationBehavior {
            position_delta_x: 3,
            position_delta_y: -2,
            min_width: Some(500),
            max_height: Some(600),
            ..Default::default()
        };
        let resolved = resolve_requested_bounds(&behavior, Bounds::new(10, 10, 400, 900));
        assert_eq!(resolved, Bounds::new(13, 8, 500, 600));
    }

    #[test]
    fn duplicate_window_ids_are_rejected() {
        let _guard = EnvGuard::set(
            r#"{"windows":[
                {"id":1,"app":"A","title":"One"},
                {"id":1,"app":"A","title":"Two"}
            ]}"#,
        );
        assert!(provider_window_infos().is_err());
    }
}
