//! Free launch previews via `md explain <flow> --json` (protocol §2).
//!
//! Explain is guaranteed engine-call-free, so the Lens variation can preview
//! the selected flow on every selection change. Cache identity follows the
//! protocol's observable axes: (path, mtimeMs, cwd, config fingerprint).
//! Because mdflow owns config resolution, cached previews are periodically
//! revalidated with `md explain`; Script Kit never reimplements its hashing.

use std::collections::HashMap;
use std::process::Command;
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

fn fetch_explain_blocking(path: &str, cwd: &str) -> ExplainState {
    let Some(binary) = mdflow_binary() else {
        return ExplainState::Failed("mdflow CLI not found on PATH".to_string());
    };
    let output = Command::new(binary)
        .arg("explain")
        .arg(path)
        .arg("--json")
        .current_dir(cwd)
        .output();
    let output = match output {
        Ok(output) => output,
        Err(err) => return ExplainState::Failed(format!("explain failed to spawn: {err}")),
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let first = stderr.lines().next().unwrap_or("explain failed");
        return ExplainState::Failed(first.to_string());
    }
    parse_explain_output(&String::from_utf8_lossy(&output.stdout))
}

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
