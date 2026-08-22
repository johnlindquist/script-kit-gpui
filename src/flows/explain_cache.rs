//! Free launch previews via `md explain <flow> --json` (protocol §2).
//!
//! Explain is guaranteed engine-call-free, so the Lens variation can preview
//! the selected flow on every selection change. Cache identity follows the
//! protocol's observable axes: (path, mtimeMs, cwd, config fingerprint).
//! Because mdflow owns config resolution, cached previews are periodically
//! revalidated with `md explain`; Script Kit never reimplements its hashing.

use std::collections::HashMap;
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use super::catalog::mdflow_binary;
use super::model::{ExplainInfo, FlowDescriptor, FLOW_UX_PROTOCOL_VERSION};

/// Keep the selected flow plus a small MRU set resolved (council guidance:
/// selected + top 3 MRU, no speculative fan-out).
const EXPLAIN_CACHE_CAPACITY: usize = 8;
/// A flow file's mtime cannot observe `.mdflow.yaml` changes. Revalidate the
/// selected preview so mdflow's authoritative fingerprint can invalidate it.
const EXPLAIN_REVALIDATE_AFTER: Duration = Duration::from_secs(5);
/// Hard deadline shared by preview and turn-argument explain calls. Ten
/// seconds matches roster fetching, but remains unratified pending human
/// sign-off (OF-38).
pub(crate) const MD_EXPLAIN_DEADLINE: Duration = Duration::from_secs(10);
const MD_EXPLAIN_POLL_INTERVAL: Duration = Duration::from_millis(5);
const MD_EXPLAIN_TERMINATE_GRACE: Duration = Duration::from_millis(50);

#[derive(Clone, PartialEq, Eq, Hash)]
struct ExplainBaseKey {
    path: String,
    mtime_ms: u64,
    cwd: String,
}

impl ExplainBaseKey {
    fn new(flow: &FlowDescriptor, cwd: &str) -> Self {
        Self {
            path: flow.path.clone(),
            mtime_ms: flow.mtime_ms,
            cwd: cwd.to_string(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct ExplainKey {
    path: String,
    mtime_ms: u64,
    cwd: String,
    config_fingerprint: String,
}

impl ExplainKey {
    fn for_base(base: &ExplainBaseKey, config_fingerprint: String) -> Self {
        Self {
            path: base.path.clone(),
            mtime_ms: base.mtime_ms,
            cwd: base.cwd.clone(),
            config_fingerprint,
        }
    }
}

struct CurrentExplain {
    key: ExplainKey,
    checked_at: Instant,
    refresh_in_flight: bool,
}

#[derive(Clone)]
pub enum ExplainState {
    Loading,
    Ready(Arc<ExplainInfo>),
    Failed(String),
}

static CACHE: Mutex<Option<Arc<ExplainCache>>> = Mutex::new(None);

pub fn explain_cache() -> Arc<ExplainCache> {
    let mut guard = CACHE.lock();
    guard
        .get_or_insert_with(|| Arc::new(ExplainCache::default()))
        .clone()
}

#[derive(Default)]
pub struct ExplainCache {
    entries: Mutex<HashMap<ExplainKey, ExplainState>>,
    current: Mutex<HashMap<ExplainBaseKey, CurrentExplain>>,
    mru: Mutex<Vec<ExplainKey>>,
    notify: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
}

impl ExplainCache {
    pub fn set_notify_hook(&self, hook: impl Fn() + Send + Sync + 'static) {
        *self.notify.lock() = Some(Box::new(hook));
    }

    fn notify(&self) {
        if let Some(hook) = self.notify.lock().as_ref() {
            hook();
        }
    }

    /// Non-blocking lookup; spawns a background resolve on a miss or bounded
    /// revalidation interval. A revalidation keeps showing the last ready
    /// preview until mdflow returns a different config fingerprint.
    pub fn state_for(self: &Arc<Self>, flow: &FlowDescriptor, cwd: &str) -> ExplainState {
        let base = ExplainBaseKey::new(flow, cwd);
        let now = Instant::now();
        let (_, state, should_fetch) = self.lookup(&base, now);

        if should_fetch {
            let cache = Arc::clone(self);
            let thread_cache = Arc::clone(self);
            let thread_base = base.clone();
            if let Err(err) = std::thread::Builder::new()
                .name("flow-explain-fetch".into())
                .spawn(move || {
                    let state = fetch_explain_blocking(&thread_base.path, &thread_base.cwd);
                    thread_cache.complete_fetch(thread_base, state, Instant::now());
                    thread_cache.notify();
                })
            {
                cache.complete_fetch(
                    base,
                    ExplainState::Failed(format!("explain worker failed to spawn: {err}")),
                    Instant::now(),
                );
                cache.notify();
            }
        }

        state
    }

    fn lookup(&self, base: &ExplainBaseKey, now: Instant) -> (ExplainKey, ExplainState, bool) {
        let mut current = self.current.lock();
        if let Some(slot) = current.get_mut(base) {
            let state = self
                .entries
                .lock()
                .get(&slot.key)
                .cloned()
                .unwrap_or(ExplainState::Loading);
            let should_fetch = !slot.refresh_in_flight
                && (matches!(&state, ExplainState::Loading)
                    || now.saturating_duration_since(slot.checked_at) >= EXPLAIN_REVALIDATE_AFTER);
            if should_fetch {
                slot.refresh_in_flight = true;
            }
            let result = (slot.key.clone(), state, should_fetch);
            self.touch_mru(&result.0);
            return result;
        }

        let key = ExplainKey::for_base(base, String::new());
        self.entries
            .lock()
            .insert(key.clone(), ExplainState::Loading);
        current.insert(
            base.clone(),
            CurrentExplain {
                key: key.clone(),
                checked_at: now,
                refresh_in_flight: true,
            },
        );
        self.touch_mru(&key);
        (key, ExplainState::Loading, true)
    }

    fn complete_fetch(&self, base: ExplainBaseKey, state: ExplainState, now: Instant) {
        let ready_fingerprint = match &state {
            ExplainState::Ready(info) => Some(info.config_fingerprint.clone()),
            ExplainState::Loading | ExplainState::Failed(_) => None,
        };

        // Keep cache identity, aliases, and MRU eviction in one critical
        // section. Every multi-lock path follows current → entries → mru.
        let mut current = self.current.lock();
        let mut entries = self.entries.lock();
        let mut mru = self.mru.lock();
        let Some(previous_key) = current.get(&base).map(|slot| slot.key.clone()) else {
            return;
        };
        if !mru.contains(&previous_key) {
            // Capacity eviction happened while this worker was in flight and
            // no later lookup re-touched it. Drop the obsolete completion;
            // worker finish order must never resurrect an evicted preview.
            current.remove(&base);
            entries.remove(&previous_key);
            return;
        }

        let landed_key = {
            let Some(slot) = current.get_mut(&base) else {
                return;
            };
            slot.checked_at = now;
            slot.refresh_in_flight = false;

            match ready_fingerprint {
                Some(Some(fingerprint)) => {
                    let new_key = ExplainKey::for_base(&base, fingerprint);
                    let preserve_hit = new_key == previous_key
                        && matches!(entries.get(&new_key), Some(ExplainState::Ready(_)));
                    if !preserve_hit {
                        if new_key != previous_key {
                            entries.remove(&previous_key);
                        }
                        entries.insert(new_key.clone(), state);
                    }
                    slot.key = new_key;
                }
                Some(None) => {
                    // A legacy fingerprint-less response cannot prove identity.
                    // Store it under an explicitly fingerprint-less key and
                    // replace it on every completed revalidation.
                    let new_key = ExplainKey::for_base(&base, String::new());
                    if new_key != previous_key {
                        entries.remove(&previous_key);
                    }
                    entries.insert(new_key.clone(), state);
                    slot.key = new_key;
                }
                None => {
                    // A failed revalidation must not discard a usable preview,
                    // but it does clear the in-flight flag so a later retry is
                    // possible. Initial failures still surface as Failed.
                    let has_ready_preview =
                        matches!(entries.get(&previous_key), Some(ExplainState::Ready(_)));
                    if !matches!(&state, ExplainState::Failed(_)) || !has_ready_preview {
                        entries.insert(previous_key.clone(), state);
                    }
                }
            }
            slot.key.clone()
        };

        replace_mru_key_in_place(&mut mru, &previous_key, &landed_key);
        if mru.len() > EXPLAIN_CACHE_CAPACITY {
            let drain_count = mru.len() - EXPLAIN_CACHE_CAPACITY;
            mru.drain(..drain_count);
            entries.retain(|key, _| mru.contains(key));
            current.retain(|_, slot| slot.refresh_in_flight || mru.contains(&slot.key));
        }
    }

    fn touch_mru(&self, key: &ExplainKey) {
        let mut mru = self.mru.lock();
        mru.retain(|k| k != key);
        mru.push(key.clone());
    }
}

/// Replace a provisional/old fingerprint key without changing its recency.
/// Worker completion order must never outrank user lookup order.
fn replace_mru_key_in_place(mru: &mut Vec<ExplainKey>, previous: &ExplainKey, landed: &ExplainKey) {
    if previous == landed {
        return;
    }
    let mut replaced = false;
    for key in mru.iter_mut() {
        if key == previous {
            *key = landed.clone();
            replaced = true;
        }
    }
    debug_assert!(replaced, "completion key must still be represented in MRU");

    let mut saw_landed = false;
    mru.retain(|key| {
        if key != landed {
            return true;
        }
        let keep = !saw_landed;
        saw_landed = true;
        keep
    });
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MdExplainJson {
    #[serde(flatten)]
    pub(crate) info: ExplainInfo,
    #[serde(default)]
    #[expect(
        dead_code,
        reason = "the mdflow wire schema retains complete template-variable metadata for compatibility"
    )]
    pub(crate) template_vars: Vec<String>,
    #[serde(default)]
    pub(crate) missing_template_vars: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct MdExplainOutput {
    pub(crate) output: std::process::Output,
    pub(crate) explain: Option<MdExplainJson>,
}

/// Run one engine-free `md explain` under an absolute deadline.
///
/// The child owns a fresh process group so timeout cleanup reaches mdflow's
/// descendants. Both output pipes are drained concurrently; timeout returns
/// only after the direct child is reaped and the readers have observed EOF.
pub(crate) fn run_md_explain_with_deadline(
    binary: &str,
    flow_path: &str,
    cwd: &str,
    explain_args: &[&str],
    deadline: Instant,
) -> std::io::Result<MdExplainOutput> {
    if Instant::now() >= deadline {
        return Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "mdflow explain deadline exhausted before spawn",
        ));
    }

    let mut command = Command::new(binary);
    command
        .arg("explain")
        .arg(flow_path)
        .args(explain_args)
        .arg("--json")
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.process_group(0);
    let mut child = command.spawn()?;
    let pgid = child.id() as libc::pid_t;

    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    let stdout_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(pipe) = stdout_pipe.as_mut() {
            let _ = pipe.read_to_end(&mut bytes);
        }
        bytes
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(pipe) = stderr_pipe.as_mut() {
            let _ = pipe.read_to_end(&mut bytes);
        }
        bytes
    });

    let status = loop {
        match child.try_wait()? {
            Some(status) => break status,
            None if Instant::now() >= deadline => {
                let mut cleanup_notes = Vec::new();
                let term_result = unsafe { libc::killpg(pgid, libc::SIGTERM) };
                if term_result != 0 {
                    let error = std::io::Error::last_os_error();
                    if error.raw_os_error() != Some(libc::ESRCH) {
                        cleanup_notes.push(format!("SIGTERM failed: {error}"));
                    }
                }

                let grace_deadline = Instant::now() + MD_EXPLAIN_TERMINATE_GRACE;
                while Instant::now() < grace_deadline {
                    if unsafe { libc::killpg(pgid, 0) } != 0
                        && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
                    {
                        break;
                    }
                    std::thread::sleep(MD_EXPLAIN_POLL_INTERVAL);
                }

                let group_alive = unsafe { libc::killpg(pgid, 0) } == 0
                    || std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH);
                if group_alive {
                    let kill_result = unsafe { libc::killpg(pgid, libc::SIGKILL) };
                    if kill_result != 0 {
                        let error = std::io::Error::last_os_error();
                        if error.raw_os_error() != Some(libc::ESRCH) {
                            cleanup_notes.push(format!("SIGKILL failed: {error}"));
                            let _ = child.kill();
                        }
                    }
                }
                if let Err(error) = child.wait() {
                    cleanup_notes.push(format!("wait failed: {error}"));
                }
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();

                let suffix = if cleanup_notes.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", cleanup_notes.join("; "))
                };
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!("mdflow explain exceeded deadline{suffix}"),
                ));
            }
            None => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                std::thread::sleep(MD_EXPLAIN_POLL_INTERVAL.min(remaining));
            }
        }
    };

    let output = std::process::Output {
        status,
        stdout: stdout_reader.join().unwrap_or_default(),
        stderr: stderr_reader.join().unwrap_or_default(),
    };
    let explain = if output.status.success() {
        Some(
            serde_json::from_slice::<MdExplainJson>(&output.stdout).map_err(|error| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("mdflow explain returned invalid JSON: {error}"),
                )
            })?,
        )
    } else {
        None
    };
    Ok(MdExplainOutput { output, explain })
}

fn fetch_explain_blocking(path: &str, cwd: &str) -> ExplainState {
    let Some(binary) = mdflow_binary() else {
        return ExplainState::Failed("mdflow CLI not found on PATH".to_string());
    };
    let output = match run_md_explain_with_deadline(
        binary,
        path,
        cwd,
        &[],
        Instant::now() + MD_EXPLAIN_DEADLINE,
    ) {
        Ok(output) => output,
        Err(err) => return ExplainState::Failed(format!("explain failed: {err}")),
    };
    if !output.output.status.success() {
        let stderr = String::from_utf8_lossy(&output.output.stderr);
        let first = stderr.lines().next().unwrap_or("explain failed");
        return ExplainState::Failed(first.to_string());
    }
    let Some(explain) = output.explain else {
        return ExplainState::Failed("explain returned no parsed JSON".to_string());
    };
    if explain.info.protocol_version == FLOW_UX_PROTOCOL_VERSION {
        ExplainState::Ready(Arc::new(explain.info))
    } else {
        ExplainState::Failed(format!(
            "unsupported explain protocol version {}",
            explain.info.protocol_version
        ))
    }
}

#[cfg(test)]
fn parse_explain_output(stdout: &str) -> ExplainState {
    match serde_json::from_str::<ExplainInfo>(stdout) {
        Ok(info) if info.protocol_version == FLOW_UX_PROTOCOL_VERSION => {
            ExplainState::Ready(Arc::new(info))
        }
        Ok(info) => ExplainState::Failed(format!(
            "unsupported explain protocol version {}",
            info.protocol_version
        )),
        Err(err) => ExplainState::Failed(format!("explain parse error: {err}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_with_fingerprint(config_fingerprint: &str) -> ExplainKey {
        ExplainKey {
            path: "/p/flows/review.md".to_string(),
            mtime_ms: 42,
            cwd: "/p".to_string(),
            config_fingerprint: config_fingerprint.to_string(),
        }
    }

    fn base_key() -> ExplainBaseKey {
        ExplainBaseKey {
            path: "/p/flows/review.md".to_string(),
            mtime_ms: 42,
            cwd: "/p".to_string(),
        }
    }

    fn ready_state(engine: &str, config_fingerprint: &str) -> ExplainState {
        parse_explain_output(&format!(
            r#"{{
                "protocolVersion": 1,
                "flowId": "project:review",
                "path": "/p/flows/review.md",
                "engine": "{engine}",
                "command": "{engine}",
                "args": [],
                "cwd": "/p",
                "prompt": "Review the diff",
                "promptTokensEstimate": 4,
                "inputs": [],
                "warnings": [],
                "configFingerprint": "{config_fingerprint}"
            }}"#
        ))
    }

    #[cfg(unix)]
    fn write_fake_md(path: &std::path::Path, body: &str) {
        use std::os::unix::fs::PermissionsExt;

        std::fs::write(path, body).expect("write fake md");
        let mut permissions = std::fs::metadata(path)
            .expect("fake md metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("make fake md executable");
    }

    #[cfg(unix)]
    fn process_exists(pid: i32) -> bool {
        let result = unsafe { libc::kill(pid, 0) };
        result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }

    #[cfg(unix)]
    fn read_fixture_pids(path: &std::path::Path) -> Vec<i32> {
        std::fs::read_to_string(path)
            .expect("fake md should persist parent and descendant pids")
            .split_whitespace()
            .map(|pid| pid.parse::<i32>().expect("numeric pid"))
            .collect()
    }

    #[cfg(unix)]
    mod of38 {
        use super::*;

        #[test]
        fn explain_deadline_kills_process_group_and_reaps_output() {
            let dir = tempfile::tempdir().expect("tempdir");
            let binary = dir.path().join("md");
            let pid_file = dir.path().join("of38-pids.txt");
            write_fake_md(
            &binary,
            "#!/bin/sh\nsleep 300 &\nchild=$!\nprintf '%s %s\\n' \"$$\" \"$child\" > of38-pids.txt\nprintf 'partial stdout\\n'\nprintf 'partial stderr\\n' >&2\nwait \"$child\"\n",
        );

            let started = Instant::now();
            let error = run_md_explain_with_deadline(
                binary.to_str().expect("utf8 binary"),
                "flow.md",
                dir.path().to_str().expect("utf8 cwd"),
                &[],
                started + Duration::from_secs(1),
            )
            .expect_err("hanging explain must time out");

            assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
            assert!(started.elapsed() < Duration::from_secs(3));
            let pids = read_fixture_pids(&pid_file);
            assert_eq!(pids.len(), 2);
            assert!(
                pids.into_iter().all(|pid| !process_exists(pid)),
                "the direct child and its descendant must both be gone after return"
            );
        }

        #[cfg(unix)]
        #[test]
        fn timed_out_explain_clears_refresh_in_flight_and_allows_retry() {
            let dir = tempfile::tempdir().expect("tempdir");
            let binary = dir.path().join("md");
            write_fake_md(
            &binary,
            "#!/bin/sh\nsleep 300 &\nchild=$!\nprintf '%s %s\\n' \"$$\" \"$child\" > of38-pids.txt\nwait \"$child\"\n",
        );

            let cache = ExplainCache::default();
            let base = ExplainBaseKey {
                path: "flow.md".into(),
                mtime_ms: 42,
                cwd: dir.path().to_string_lossy().into_owned(),
            };
            let first_check = Instant::now();
            assert!(cache.lookup(&base, first_check).2);
            let timeout = run_md_explain_with_deadline(
                binary.to_str().expect("utf8 binary"),
                &base.path,
                &base.cwd,
                &[],
                Instant::now() + Duration::from_millis(50),
            )
            .expect_err("first explain should time out");
            let timeout_completed_at = Instant::now();
            cache.complete_fetch(
                base.clone(),
                ExplainState::Failed(timeout.to_string()),
                timeout_completed_at,
            );
            cache.notify();
            assert!(
                !cache
                    .current
                    .lock()
                    .get(&base)
                    .expect("current slot")
                    .refresh_in_flight
            );

            let retry_at =
                timeout_completed_at + EXPLAIN_REVALIDATE_AFTER + Duration::from_millis(1);
            let (_, failed, should_retry) = cache.lookup(&base, retry_at);
            assert!(matches!(failed, ExplainState::Failed(_)));
            assert!(should_retry, "a completed timeout must permit revalidation");

            write_fake_md(
            &binary,
            "#!/bin/sh\nprintf '%s\\n' '{\"protocolVersion\":1,\"flowId\":\"project:retry\",\"path\":\"flow.md\",\"engine\":\"pi\",\"command\":\"pi\",\"args\":[],\"cwd\":\".\",\"prompt\":\"ok\",\"promptTokensEstimate\":1,\"inputs\":[],\"warnings\":[],\"configFingerprint\":\"sha256:retry\",\"templateVars\":[\"_1\"],\"missingTemplateVars\":[]}'\n",
        );
            let output = run_md_explain_with_deadline(
                binary.to_str().expect("utf8 binary"),
                &base.path,
                &base.cwd,
                &[],
                Instant::now() + Duration::from_secs(1),
            )
            .expect("retry explain succeeds");
            let explain = output
                .explain
                .expect("successful explain returns parsed JSON");
            assert_eq!(explain.template_vars, ["_1"]);
            assert!(explain.missing_template_vars.is_empty());
            cache.complete_fetch(
                base.clone(),
                ExplainState::Ready(Arc::new(explain.info)),
                retry_at,
            );
            cache.notify();
            let (_, retried, should_retry_again) = cache.lookup(&base, retry_at);
            assert!(!should_retry_again);
            assert!(matches!(retried, ExplainState::Ready(info) if info.engine == "pi"));
        }
    }

    #[test]
    fn changed_config_fingerprint_misses_the_cached_preview() {
        let cache = ExplainCache::default();
        let base = base_key();
        let first_check = Instant::now();
        assert!(cache.lookup(&base, first_check).2);
        cache.complete_fetch(
            base.clone(),
            ready_state("codex", "sha256:old"),
            first_check,
        );

        let revalidate_at = first_check + EXPLAIN_REVALIDATE_AFTER;
        let (_, stale_state, should_refetch) = cache.lookup(&base, revalidate_at);
        assert!(should_refetch);
        assert!(matches!(
            stale_state,
            ExplainState::Ready(info) if info.engine == "codex"
        ));
        cache.complete_fetch(
            base.clone(),
            ready_state("claude", "sha256:new"),
            revalidate_at,
        );

        let (key, refreshed_state, should_refetch) = cache.lookup(&base, revalidate_at);
        assert!(!should_refetch);
        assert!(key == key_with_fingerprint("sha256:new"));
        assert!(matches!(
            refreshed_state,
            ExplainState::Ready(info) if info.engine == "claude"
        ));
        assert_eq!(cache.entries.lock().len(), 1);
        assert!(!cache
            .entries
            .lock()
            .contains_key(&key_with_fingerprint("sha256:old")));
        assert_eq!(cache.mru.lock().len(), 1);
    }

    #[test]
    fn unchanged_config_fingerprint_preserves_the_cached_preview_hit() {
        let cache = ExplainCache::default();
        let base = base_key();
        let first_check = Instant::now();
        assert!(cache.lookup(&base, first_check).2);
        cache.complete_fetch(
            base.clone(),
            ready_state("codex", "sha256:same"),
            first_check,
        );
        let (_, first_state, _) = cache.lookup(&base, first_check);
        let ExplainState::Ready(first_info) = first_state else {
            panic!("expected first ready preview");
        };

        let revalidate_at = first_check + EXPLAIN_REVALIDATE_AFTER;
        assert!(cache.lookup(&base, revalidate_at).2);
        cache.complete_fetch(
            base.clone(),
            ready_state("codex", "sha256:same"),
            revalidate_at,
        );

        let (key, unchanged_state, should_refetch) = cache.lookup(&base, revalidate_at);
        let ExplainState::Ready(unchanged_info) = unchanged_state else {
            panic!("expected unchanged ready preview");
        };
        assert!(!should_refetch);
        assert!(key == key_with_fingerprint("sha256:same"));
        assert!(Arc::ptr_eq(&first_info, &unchanged_info));
        assert_eq!(cache.entries.lock().len(), 1);
        assert_eq!(cache.mru.lock().len(), 1);
    }

    #[test]
    fn fingerprintless_revalidation_cannot_reuse_a_stale_fingerprint_identity() {
        let cache = ExplainCache::default();
        let base = base_key();
        let first_check = Instant::now();
        assert!(cache.lookup(&base, first_check).2);
        cache.complete_fetch(
            base.clone(),
            ready_state("codex", "sha256:old"),
            first_check,
        );

        let revalidate_at = first_check + EXPLAIN_REVALIDATE_AFTER;
        assert!(cache.lookup(&base, revalidate_at).2);
        let mut fingerprintless = ready_state("pi", "sha256:placeholder");
        let ExplainState::Ready(info) = &mut fingerprintless else {
            panic!("expected ready preview");
        };
        Arc::make_mut(info).config_fingerprint = None;
        cache.complete_fetch(base.clone(), fingerprintless, revalidate_at);

        let (key, state, should_refetch) = cache.lookup(&base, revalidate_at);
        assert!(!should_refetch);
        assert!(key.config_fingerprint.is_empty());
        assert!(matches!(state, ExplainState::Ready(info) if info.engine == "pi"));
        assert!(!cache
            .entries
            .lock()
            .contains_key(&key_with_fingerprint("sha256:old")));
    }

    #[test]
    fn late_worker_completion_cannot_evict_the_most_recent_lookup() {
        let cache = ExplainCache::default();
        let now = Instant::now();
        let bases: Vec<ExplainBaseKey> = (0..=EXPLAIN_CACHE_CAPACITY)
            .map(|index| ExplainBaseKey {
                path: format!("/p/flows/{index}.md"),
                mtime_ms: index as u64,
                cwd: "/p".to_string(),
            })
            .collect();
        for base in &bases {
            assert!(cache.lookup(base, now).2);
        }

        for (index, base) in bases.iter().enumerate().rev() {
            cache.complete_fetch(
                base.clone(),
                ready_state("codex", &format!("sha256:{index}")),
                now,
            );
        }

        let most_recent = bases.last().expect("most recent base");
        let evicted_oldest = bases.first().expect("oldest base");
        assert!(cache.current.lock().contains_key(most_recent));
        assert!(!cache.current.lock().contains_key(evicted_oldest));
        assert_eq!(
            cache.mru.lock().last().map(|key| key.path.as_str()),
            Some(most_recent.path.as_str())
        );
        assert!(!cache
            .mru
            .lock()
            .iter()
            .any(|key| key.path == evicted_oldest.path));
        assert!(cache
            .entries
            .lock()
            .keys()
            .any(|key| key.path == most_recent.path));
        assert!(!cache
            .entries
            .lock()
            .keys()
            .any(|key| key.path == evicted_oldest.path));
        assert_eq!(cache.mru.lock().len(), EXPLAIN_CACHE_CAPACITY);
        assert_eq!(cache.entries.lock().len(), EXPLAIN_CACHE_CAPACITY);
    }

    #[test]
    fn parse_explain_output_round_trips_protocol_v1() {
        let state = parse_explain_output(
            r#"{
                "protocolVersion": 1,
                "flowId": "project:review",
                "path": "/p/flows/review.md",
                "engine": "pi",
                "command": "pi",
                "args": ["--print"],
                "cwd": "/p",
                "prompt": "Review the diff",
                "promptTokensEstimate": 4,
                "inputs": [],
                "warnings": [],
                "configFingerprint": "sha256:abc"
            }"#,
        );
        match state {
            ExplainState::Ready(info) => {
                assert_eq!(info.engine, "pi");
                assert_eq!(info.args, vec!["--print".to_string()]);
                assert_eq!(info.config_fingerprint.as_deref(), Some("sha256:abc"));
            }
            _ => panic!("expected Ready"),
        }
    }

    #[test]
    fn parse_explain_output_rejects_future_protocol() {
        let state = parse_explain_output(
            r#"{"protocolVersion":9,"flowId":"x","path":"/p","engine":"pi","command":"pi","args":[],"cwd":"/p","prompt":"","promptTokensEstimate":0,"inputs":[],"warnings":[]}"#,
        );
        assert!(matches!(state, ExplainState::Failed(_)));
    }

    #[test]
    fn parse_explain_output_reports_garbage() {
        assert!(matches!(
            parse_explain_output("nope"),
            ExplainState::Failed(_)
        ));
    }
}
