//! Process-wide effect boundary for the owned production evaluator.
//!
//! The binary re-exports this library module so both compilations share one
//! irreversible policy. This is an application guard, not an OS sandbox.

use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WindowHostPolicy {
    #[default]
    Interactive,
    OwnedHidden,
}

impl WindowHostPolicy {
    pub fn validate(self) -> Result<(), EffectRefusal> {
        match (self, is_owned_evaluation()) {
            (Self::Interactive, false) | (Self::OwnedHidden, true) => Ok(()),
            (Self::Interactive, true) => Err(EffectRefusal {
                code: "interactive_host_forbidden",
            }),
            (Self::OwnedHidden, false) => Err(EffectRefusal {
                code: "owned_host_policy_missing",
            }),
        }
    }

    pub const fn is_hidden(self) -> bool {
        matches!(self, Self::OwnedHidden)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalEffect {
    Process,
    Provider,
    Credentials,
    SystemClipboard,
    NativeInput,
    NativeVisibility,
    ScreenCapture,
    Device,
    GlobalMonitor,
    Notification,
    OpenExternal,
    ExternalStorage,
    SystemDiscovery,
}

impl ExternalEffect {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Process => "external_process_forbidden",
            Self::Provider => "provider_forbidden",
            Self::Credentials => "credentials_forbidden",
            Self::SystemClipboard => "system_clipboard_forbidden",
            Self::NativeInput => "native_input_forbidden",
            Self::NativeVisibility => "native_visibility_forbidden",
            Self::ScreenCapture => "screen_capture_forbidden",
            Self::Device => "device_forbidden",
            Self::GlobalMonitor => "global_monitor_forbidden",
            Self::Notification => "notification_forbidden",
            Self::OpenExternal => "external_open_forbidden",
            Self::ExternalStorage => "external_storage_forbidden",
            Self::SystemDiscovery => "system_discovery_forbidden",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectRefusal {
    pub code: &'static str,
}

impl std::fmt::Display for EffectRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code)
    }
}

impl std::error::Error for EffectRefusal {}

/// Only the most recently copied text is retained by the owned process.
pub const MAX_OWNED_COPY_TEXT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CopyDestination {
    SystemClipboard,
    OwnedProcessLocal,
}

/// Immutable evidence of a completed text write, not a clipboard read receipt.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyReceipt {
    destination: CopyDestination,
    byte_length: usize,
    sha256: String,
    revision: Option<u64>,
}

impl CopyReceipt {
    fn new(text: &str, destination: CopyDestination, revision: Option<u64>) -> Self {
        Self {
            destination,
            byte_length: text.len(),
            sha256: format!("{:x}", Sha256::digest(text.as_bytes())),
            revision,
        }
    }

    pub fn destination(&self) -> CopyDestination {
        self.destination
    }

    pub fn byte_length(&self) -> usize {
        self.byte_length
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    pub fn revision(&self) -> Option<u64> {
        self.revision
    }

    /// Preserve normal feedback while explicitly identifying owned local copies.
    pub fn feedback(&self, message: String) -> String {
        match self.destination {
            CopyDestination::SystemClipboard => message,
            CopyDestination::OwnedProcessLocal => format!("{message} (process-local)"),
        }
    }
}

/// Shared routing for the binary and library clipboard owner. The callback must
/// perform a real system write; it is never invoked for an owned policy.
pub fn route_text_copy(
    policy: Option<&OwnedEvaluationPolicy>,
    text: &str,
    write_system: impl FnOnce(&str) -> Result<(), String>,
) -> Result<CopyReceipt, String> {
    if let Some(policy) = policy {
        return policy
            .store_owned_copy(text)
            .map_err(|error| error.to_string());
    }
    write_system(text)?;
    Ok(CopyReceipt::new(
        text,
        CopyDestination::SystemClipboard,
        None,
    ))
}

/// Inspection is limited to text actually stored in this policy's local sink.
/// This never reads, emulates, or exposes the system clipboard.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnedTextCopy {
    text: String,
    receipt: CopyReceipt,
}

impl OwnedTextCopy {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn receipt(&self) -> &CopyReceipt {
        &self.receipt
    }
}

#[derive(Debug)]
pub struct OwnedEvaluationPolicy {
    root: PathBuf,
    process_instance_id: String,
    session_generation: String,
    refused_effects: AtomicU64,
    completed_fixture_effects: AtomicU64,
    copied_text: Mutex<Option<OwnedTextCopy>>,
    root_search_clock: Mutex<Option<OwnedRootSearchClock>>,
}

static OWNED_EVALUATION: OnceLock<OwnedEvaluationPolicy> = OnceLock::new();

impl OwnedEvaluationPolicy {
    pub fn new(
        root: &Path,
        process_instance_id: String,
        session_generation: String,
    ) -> Result<Self, EffectRefusal> {
        if !root.is_absolute()
            || root
                .components()
                .any(|part| matches!(part, Component::ParentDir))
            || process_instance_id.is_empty()
            || session_generation.is_empty()
        {
            return Err(EffectRefusal {
                code: "invalid_evaluation_identity",
            });
        }
        let canonical = root.canonicalize().map_err(|_| EffectRefusal {
            code: "evaluation_root_unavailable",
        })?;
        if canonical != root || !canonical.is_dir() {
            return Err(EffectRefusal {
                code: "evaluation_root_not_canonical",
            });
        }
        Ok(Self {
            root: canonical,
            process_instance_id,
            session_generation,
            refused_effects: AtomicU64::new(0),
            completed_fixture_effects: AtomicU64::new(0),
            copied_text: Mutex::new(None),
            root_search_clock: Mutex::new(None),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn process_instance_id(&self) -> &str {
        &self.process_instance_id
    }

    pub fn session_generation(&self) -> &str {
        &self.session_generation
    }

    /// Store an exact, bounded text copy without obtaining any native handle.
    /// Rejected writes leave the prior text, revision and completion count intact.
    pub fn store_owned_copy(&self, text: &str) -> Result<CopyReceipt, EffectRefusal> {
        if text.len() > MAX_OWNED_COPY_TEXT_BYTES {
            return Err(EffectRefusal {
                code: "owned_copy_text_too_large",
            });
        }
        let mut copied_text = self.copied_text.lock().map_err(|_| EffectRefusal {
            code: "owned_copy_sink_unavailable",
        })?;
        let revision = copied_text
            .as_ref()
            .and_then(|copy| copy.receipt.revision)
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(EffectRefusal {
                code: "owned_copy_revision_exhausted",
            })?;
        let receipt = CopyReceipt::new(text, CopyDestination::OwnedProcessLocal, Some(revision));
        *copied_text = Some(OwnedTextCopy {
            text: text.to_owned(),
            receipt: receipt.clone(),
        });
        self.completed_fixture_effects
            .fetch_add(1, Ordering::Relaxed);
        Ok(receipt)
    }

    /// Return one coherent snapshot of the latest owned text and its receipt.
    /// `None` means no local copy has completed; it is not an empty clipboard.
    pub fn owned_copy_snapshot(&self) -> Result<Option<OwnedTextCopy>, EffectRefusal> {
        self.copied_text
            .lock()
            .map(|copy| copy.clone())
            .map_err(|_| EffectRefusal {
                code: "owned_copy_sink_unavailable",
            })
    }

    /// Validate each existing ancestor without following a symlink. New leaf
    /// files are allowed only underneath the already-owned canonical root.
    pub fn require_owned_path(&self, path: &Path) -> Result<(), EffectRefusal> {
        let relative = path.strip_prefix(&self.root).map_err(|_| EffectRefusal {
            code: "evaluation_path_outside_owner",
        })?;
        let mut current = self.root.clone();
        for part in relative.components() {
            let Component::Normal(name) = part else {
                return Err(EffectRefusal {
                    code: "evaluation_path_not_normalized",
                });
            };
            current.push(name);
            match std::fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(EffectRefusal {
                        code: "evaluation_path_symlink",
                    });
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => {
                    return Err(EffectRefusal {
                        code: "evaluation_path_unavailable",
                    });
                }
            }
        }
        Ok(())
    }

    pub fn refused_effect_count(&self) -> u64 {
        self.refused_effects.load(Ordering::Relaxed)
    }

    pub fn completed_fixture_effect_count(&self) -> u64 {
        self.completed_fixture_effects.load(Ordering::Relaxed)
    }
}

/// Called only by the early evaluator bootstrap, before constructing services.
/// A process cannot remove or replace its installed policy.
pub fn install_owned_evaluation(policy: OwnedEvaluationPolicy) -> Result<(), EffectRefusal> {
    if !cfg!(feature = "owned-ui-evaluation") {
        return Err(EffectRefusal {
            code: "evaluation_feature_unavailable",
        });
    }
    OWNED_EVALUATION.set(policy).map_err(|_| EffectRefusal {
        code: "evaluation_policy_already_installed",
    })
}

pub fn owned_evaluation() -> Option<&'static OwnedEvaluationPolicy> {
    OWNED_EVALUATION.get()
}

pub fn is_owned_evaluation() -> bool {
    OWNED_EVALUATION.get().is_some()
}

/// 2026-05-01 01:00 UTC, one hour after the compiled search fixture timestamps.
/// Display time stays held while the separate source-freshness clock advances.
pub const OWNED_ROOT_SEARCH_DISPLAY_UNIX_MS: i64 = 1_777_597_200_000;

/// Logical source freshness and a held display wall clock. GPUI timers,
/// animations and measured latency retain their existing clocks.
/// Enabling twice never rewinds a prepared case.
#[derive(Debug)]
struct OwnedRootSearchClock {
    now: Instant,
    advanced: Duration,
    enabled: bool,
}

impl OwnedRootSearchClock {
    fn display_unix_ms(&self) -> Option<i64> {
        self.enabled.then_some(OWNED_ROOT_SEARCH_DISPLAY_UNIX_MS)
    }

    fn advance(&mut self, delta: Duration) -> Result<(), EffectRefusal> {
        if !self.enabled {
            return Err(EffectRefusal {
                code: "root_search_clock_not_enabled",
            });
        }
        let advanced = self
            .advanced
            .checked_add(delta)
            .filter(|total| *total <= Duration::from_secs(24 * 60 * 60))
            .ok_or(EffectRefusal {
                code: "root_search_clock_limit",
            })?;
        let now = self.now.checked_add(delta).ok_or(EffectRefusal {
            code: "root_search_clock_limit",
        })?;
        self.now = now;
        self.advanced = advanced;
        Ok(())
    }
}

#[expect(
    clippy::expect_used,
    reason = "A poisoned owned clock must fail closed, never fall back to native time."
)]
pub fn root_search_now() -> Instant {
    if let Some(policy) = owned_evaluation() {
        let clock = policy
            .root_search_clock
            .lock()
            .expect("owned_root_search_clock_poisoned");
        if let Some(clock) = clock.as_ref().filter(|clock| clock.enabled) {
            return clock.now;
        }
    }
    Instant::now()
}

/// Use real wall time in production and the declared fixture time in owned search.
#[expect(
    clippy::expect_used,
    reason = "A poisoned owned clock must fail closed, never invent a display timestamp."
)]
pub fn root_search_display_unix_ms() -> i64 {
    if let Some(policy) = owned_evaluation() {
        let clock = policy
            .root_search_clock
            .lock()
            .expect("owned_root_search_clock_poisoned");
        if let Some(now) = clock
            .as_ref()
            .and_then(OwnedRootSearchClock::display_unix_ms)
        {
            return now;
        }
    }
    chrono::Utc::now().timestamp_millis()
}

pub fn enable_owned_root_search_clock() -> Result<(), EffectRefusal> {
    let policy = owned_evaluation().ok_or(EffectRefusal {
        code: "owned_runtime_required",
    })?;
    let mut clock = policy.root_search_clock.lock().map_err(|_| EffectRefusal {
        code: "owned_root_search_clock_poisoned",
    })?;
    let clock = clock.get_or_insert_with(|| OwnedRootSearchClock {
        now: Instant::now(),
        advanced: Duration::ZERO,
        enabled: true,
    });
    if !clock.enabled {
        clock.now = clock.now.max(Instant::now());
        clock.enabled = true;
    }
    Ok(())
}

pub fn advance_owned_root_search_clock(delta: Duration) -> Result<(), EffectRefusal> {
    let policy = owned_evaluation().ok_or(EffectRefusal {
        code: "owned_runtime_required",
    })?;
    let mut clock = policy.root_search_clock.lock().map_err(|_| EffectRefusal {
        code: "owned_root_search_clock_poisoned",
    })?;
    clock
        .as_mut()
        .ok_or(EffectRefusal {
            code: "root_search_clock_not_enabled",
        })?
        .advance(delta)
}

pub fn disable_owned_root_search_clock() -> Result<(), EffectRefusal> {
    let policy = owned_evaluation().ok_or(EffectRefusal {
        code: "owned_runtime_required",
    })?;
    let mut clock = policy.root_search_clock.lock().map_err(|_| EffectRefusal {
        code: "owned_root_search_clock_poisoned",
    })?;
    if let Some(clock) = clock.as_mut() {
        clock.enabled = false;
    }
    Ok(())
}

#[cfg(test)]
mod root_search_clock_tests {
    use super::*;

    #[test]
    fn logical_source_time_advances_monotonically_and_refuses_overflow_atomically() {
        let start = Instant::now();
        let mut clock = OwnedRootSearchClock {
            now: start,
            advanced: Duration::ZERO,
            enabled: true,
        };
        let display_time = clock.display_unix_ms();
        assert_eq!(display_time, Some(OWNED_ROOT_SEARCH_DISPLAY_UNIX_MS));
        clock.advance(Duration::from_secs(30)).unwrap();
        assert_eq!(clock.now.duration_since(start), Duration::from_secs(30));
        assert_eq!(
            clock
                .advance(Duration::from_secs(24 * 60 * 60))
                .unwrap_err()
                .code,
            "root_search_clock_limit"
        );
        assert_eq!(clock.now.duration_since(start), Duration::from_secs(30));
        assert_eq!(clock.advanced, Duration::from_secs(30));
        assert_eq!(clock.display_unix_ms(), display_time);
        clock.enabled = false;
        assert_eq!(clock.display_unix_ms(), None);
        assert_eq!(
            clock.advance(Duration::ZERO).unwrap_err().code,
            "root_search_clock_not_enabled"
        );
    }

    #[test]
    fn source_clock_controls_require_installed_runtime_authority() {
        assert_eq!(
            enable_owned_root_search_clock().unwrap_err().code,
            "owned_runtime_required"
        );
        assert_eq!(
            advance_owned_root_search_clock(Duration::ZERO)
                .unwrap_err()
                .code,
            "owned_runtime_required"
        );
        assert_eq!(
            disable_owned_root_search_clock().unwrap_err().code,
            "owned_runtime_required"
        );
    }
}

/// Check at the actual effect boundary, before obtaining native handles,
/// credentials or operator data. A refusal must not be reported as success.
pub fn check(effect: ExternalEffect) -> Result<(), EffectRefusal> {
    if let Some(policy) = OWNED_EVALUATION.get() {
        policy.refused_effects.fetch_add(1, Ordering::Relaxed);
        return Err(EffectRefusal {
            code: effect.code(),
        });
    }
    Ok(())
}

/// Record only after a permitted fixture sink actually completed its effect.
pub fn record_completed_fixture_effect() {
    if let Some(policy) = OWNED_EVALUATION.get() {
        policy
            .completed_fixture_effects
            .fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod copy_tests {
    use super::*;

    fn policy() -> (tempfile::TempDir, OwnedEvaluationPolicy) {
        let root = tempfile::tempdir().expect("owned test root");
        let canonical = root.path().canonicalize().expect("canonical test root");
        let policy = OwnedEvaluationPolicy::new(
            &canonical,
            "copy-test-process".to_owned(),
            "copy-test-session".to_owned(),
        )
        .expect("explicit owned policy");
        (root, policy)
    }

    #[test]
    fn owned_copy_stores_exact_text_without_calling_system_writer() {
        let (_root, policy) = policy();
        assert_eq!(policy.owned_copy_snapshot().unwrap(), None);
        let text = "\u{1f469}\u{200d}\u{1f4bb}\0\nSDK reference\r\n";
        let receipt = route_text_copy(Some(&policy), text, |_| {
            panic!("owned copy must not enter the system clipboard writer")
        })
        .unwrap();
        let snapshot = policy.owned_copy_snapshot().unwrap().unwrap();
        assert_eq!(snapshot.text(), text);
        assert_eq!(snapshot.receipt(), &receipt);
        assert_eq!(receipt.destination(), CopyDestination::OwnedProcessLocal);
        assert_eq!(receipt.byte_length(), text.len());
        assert_eq!(receipt.revision(), Some(1));
        assert_eq!(
            receipt.feedback("Copied reference".to_owned()),
            "Copied reference (process-local)"
        );
        assert_eq!(policy.completed_fixture_effect_count(), 1);
        assert_eq!(policy.refused_effect_count(), 0);
    }

    #[test]
    fn receipt_hash_and_serialized_owned_inspection_describe_real_text() {
        let (_root, policy) = policy();
        let receipt = policy.store_owned_copy("abc").unwrap();
        assert_eq!(
            receipt.sha256(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let snapshot = policy.owned_copy_snapshot().unwrap().unwrap();
        assert_eq!(
            serde_json::to_value(snapshot).unwrap(),
            serde_json::json!({
                "text": "abc",
                "receipt": {
                    "destination": "ownedProcessLocal",
                    "byteLength": 3,
                    "sha256": receipt.sha256(),
                    "revision": 1,
                },
            })
        );
    }

    #[test]
    fn each_real_write_revisions_even_identical_or_empty_text_and_preserves_old_receipts() {
        let (_root, policy) = policy();
        let first = policy.store_owned_copy("abc").unwrap();
        let first_snapshot = policy.owned_copy_snapshot().unwrap().unwrap();
        let second = policy.store_owned_copy("abc").unwrap();
        let third = policy.store_owned_copy("").unwrap();
        assert_eq!(first.revision(), Some(1));
        assert_eq!(second.revision(), Some(2));
        assert_eq!(third.revision(), Some(3));
        assert_eq!(third.byte_length(), 0);
        assert_eq!(policy.owned_copy_snapshot().unwrap().unwrap().text(), "");
        assert_eq!(first_snapshot.text(), "abc");
        assert_eq!(first_snapshot.receipt(), &first);
        assert_eq!(policy.completed_fixture_effect_count(), 3);
    }

    #[test]
    fn byte_bound_accepts_exact_limit_and_rejects_overflow_without_completion() {
        let (_root, policy) = policy();
        let mut text = "\u{e9}".repeat(MAX_OWNED_COPY_TEXT_BYTES / 2);
        let receipt = policy.store_owned_copy(&text).unwrap();
        assert_eq!(receipt.byte_length(), MAX_OWNED_COPY_TEXT_BYTES);
        let before = policy.owned_copy_snapshot().unwrap();
        text.push('x');
        assert_eq!(
            route_text_copy(Some(&policy), &text, |_| {
                panic!("oversized owned text must not fall through to system clipboard")
            })
            .unwrap_err(),
            "owned_copy_text_too_large"
        );
        assert_eq!(policy.owned_copy_snapshot().unwrap(), before);
        assert_eq!(policy.completed_fixture_effect_count(), 1);
        assert_eq!(policy.store_owned_copy("next").unwrap().revision(), Some(2));
    }

    #[test]
    fn normal_copy_calls_real_writer_contract_once_and_does_not_claim_local_storage() {
        let mut writes = Vec::new();
        let receipt = route_text_copy(None, "normal text", |text| {
            writes.push(text.to_owned());
            Ok(())
        })
        .unwrap();
        assert_eq!(writes, ["normal text"]);
        assert_eq!(receipt.destination(), CopyDestination::SystemClipboard);
        assert_eq!(receipt.revision(), None);
        assert_eq!(receipt.byte_length(), "normal text".len());
        assert_eq!(
            receipt.feedback("Copied reference".to_owned()),
            "Copied reference"
        );
        let error = route_text_copy(None, "failure", |_| Err("writer failed".to_owned()));
        assert_eq!(error.unwrap_err(), "writer failed");
    }

    #[test]
    fn explicit_policies_do_not_share_copied_text_or_completion() {
        let (_first_root, first) = policy();
        let (_second_root, second) = policy();
        first.store_owned_copy("first").unwrap();
        assert_eq!(second.owned_copy_snapshot().unwrap(), None);
        assert_eq!(second.completed_fixture_effect_count(), 0);
        assert_eq!(
            second.store_owned_copy("second").unwrap().revision(),
            Some(1)
        );
        assert_eq!(
            first.owned_copy_snapshot().unwrap().unwrap().text(),
            "first"
        );
        assert_eq!(
            second.owned_copy_snapshot().unwrap().unwrap().text(),
            "second"
        );
    }

    #[test]
    fn concurrent_owned_writes_have_unique_monotonic_revisions_and_matching_snapshot() {
        let (_root, policy) = policy();
        let receipts = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|index| {
                    let policy = &policy;
                    scope.spawn(move || policy.store_owned_copy(&format!("copy-{index}")).unwrap())
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect::<Vec<_>>()
        });
        let mut revisions: Vec<_> = receipts
            .iter()
            .map(|receipt| receipt.revision().unwrap())
            .collect();
        revisions.sort_unstable();
        assert_eq!(revisions, (1..=8).collect::<Vec<_>>());
        let snapshot = policy.owned_copy_snapshot().unwrap().unwrap();
        assert_eq!(snapshot.receipt().revision(), Some(8));
        assert_eq!(
            snapshot.receipt().sha256(),
            format!("{:x}", Sha256::digest(snapshot.text().as_bytes()))
        );
        assert_eq!(policy.completed_fixture_effect_count(), 8);
    }

    #[test]
    fn revision_exhaustion_fails_without_overwriting_or_counting_completion() {
        let (_root, policy) = policy();
        policy.store_owned_copy("kept").unwrap();
        policy
            .copied_text
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .receipt
            .revision = Some(u64::MAX);
        let before = policy.owned_copy_snapshot().unwrap();
        assert_eq!(
            policy.store_owned_copy("rejected").unwrap_err().code,
            "owned_copy_revision_exhausted"
        );
        assert_eq!(policy.owned_copy_snapshot().unwrap(), before);
        assert_eq!(policy.completed_fixture_effect_count(), 1);
    }
}
