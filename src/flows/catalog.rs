//! Flow catalog: invokes `md roster --json` and caches results per cwd.
//!
//! mdflow owns discovery — the app never re-implements project-root walking
//! or frontmatter parsing (protocol §1). The cache is invalidated by cwd
//! change or age; refreshes run on background threads and land via the
//! registry-style notify hook so renderers stay passive.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use super::model::{FlowDescriptor, RosterSnapshot, FLOW_UX_PROTOCOL_VERSION};

/// Roster entries older than this refetch on next access.
const ROSTER_TTL: Duration = Duration::from_secs(10);
/// Hard deadline on one `md roster --json` run. Without it a hung mdflow
/// pins the cwd's entry at Loading forever (spawn_refresh refuses to stack
/// a second fetch), permanently wedging flow discovery.
const ROSTER_FETCH_DEADLINE: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RosterStatus {
    Ready,
    Loading,
    /// mdflow present but pre-protocol; only terminal `--json` runs work.
    Legacy,
    Error,
}

impl RosterStatus {
    pub fn automation_label(self) -> &'static str {
        match self {
            RosterStatus::Ready => "ready",
            RosterStatus::Loading => "loading",
            RosterStatus::Legacy => "legacy",
            RosterStatus::Error => "error",
        }
    }
}

#[derive(Clone)]
pub struct RosterEntry {
    pub status: RosterStatus,
    pub flows: Arc<Vec<FlowDescriptor>>,
    pub warnings: Vec<String>,
    pub fetched_at: Instant,
}

impl RosterEntry {
    fn empty(status: RosterStatus) -> Self {
        Self {
            status,
            flows: Arc::new(Vec::new()),
            warnings: Vec::new(),
            fetched_at: Instant::now(),
        }
    }

    pub fn is_stale(&self) -> bool {
        self.fetched_at.elapsed() > ROSTER_TTL
    }
}

static CATALOG: Mutex<Option<Arc<FlowCatalog>>> = Mutex::new(None);

/// Monotonic counter bumped whenever any roster entry lands. Main-menu
/// result caches poll this to notice async roster arrivals without a
/// cx handle (the desk repaints via its tick loop instead).
static ROSTER_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn roster_generation() -> u64 {
    ROSTER_GENERATION.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn flow_catalog() -> Arc<FlowCatalog> {
    let mut guard = CATALOG.lock();
    guard
        .get_or_insert_with(|| Arc::new(FlowCatalog::default()))
        .clone()
}

/// Resolve the mdflow binary, preferring `mdflow` over `md` (`md` may shadow
/// other tools on some systems; the long name is unambiguous).
pub fn mdflow_binary() -> Option<&'static str> {
    // Success is cached forever; a miss is re-probed on every call so
    // installing mdflow while the app is open starts working immediately
    // (a cached "not found" was a permanent dead end until relaunch).
    static RESOLVED: Mutex<Option<&'static str>> = Mutex::new(None);
    let mut guard = RESOLVED.lock();
    if guard.is_some() {
        return *guard;
    }
    let found = if which::which("mdflow").is_ok() {
        Some("mdflow")
    } else if which::which("md").is_ok() {
        Some("md")
    } else {
        None
    };
    if found.is_some() {
        *guard = found;
    }
    found
}

#[derive(Default)]
pub struct FlowCatalog {
    entries: Mutex<HashMap<String, RosterEntry>>,
    notify: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
}

impl FlowCatalog {
    pub fn set_notify_hook(&self, hook: impl Fn() + Send + Sync + 'static) {
        *self.notify.lock() = Some(Box::new(hook));
    }

    fn notify(&self) {
        if let Some(hook) = self.notify.lock().as_ref() {
            hook();
        }
    }

    /// Current entry for a cwd without blocking; kicks off a background
    /// refresh when missing or stale. Renderers call this every frame.
    pub fn roster_for(self: &Arc<Self>, cwd: &str) -> RosterEntry {
        let needs_refresh = {
            let entries = self.entries.lock();
            entry_needs_refresh(entries.get(cwd))
        };
        if needs_refresh {
            self.spawn_refresh(cwd);
        }
        self.entries
            .lock()
            .get(cwd)
            .cloned()
            .unwrap_or_else(|| RosterEntry::empty(RosterStatus::Loading))
    }

    /// Force refresh (cwd chip changed, manual reload action).
    pub fn refresh(self: &Arc<Self>, cwd: &str) {
        self.spawn_refresh(cwd);
    }

    /// Cheap staleness check without cloning the entry: spawns a background
    /// refresh when the cwd's roster is stale or missing. Main-menu cache
    /// getters call this every read so a hot cache can never pin a stale
    /// roster forever (the refresh completion bumps the generation, which
    /// invalidates those caches and repaints via the notify hook).
    pub fn poke(self: &Arc<Self>, cwd: &str) {
        let needs_refresh = {
            let entries = self.entries.lock();
            entry_needs_refresh(entries.get(cwd))
        };
        if needs_refresh {
            self.spawn_refresh(cwd);
        }
    }

    fn spawn_refresh(self: &Arc<Self>, cwd: &str) {
        {
            let mut entries = self.entries.lock();
            let placeholder = entries
                .entry(cwd.to_string())
                .or_insert_with(|| RosterEntry::empty(RosterStatus::Loading));
            if placeholder.status == RosterStatus::Loading && !placeholder.flows.is_empty() {
                return; // refresh already in flight with previous data showing
            }
            placeholder.status = RosterStatus::Loading;
        }

        // Unit tests run under GPUI's deterministic scheduler. An OS thread
        // completing a roster fetch during an unrelated GPUI test makes that
        // test nondeterministic, so keep test refreshes local and side-effect
        // free. Fetch parsing and cache-completion behavior have direct unit
        // coverage below; production retains the asynchronous process path.
        #[cfg(test)]
        {
            self.complete_refresh(cwd.to_string(), RosterEntry::empty(RosterStatus::Ready));
            return;
        }

        #[cfg(not(test))]
        {
            let catalog = Arc::clone(self);
            let cwd = cwd.to_string();
            std::thread::Builder::new()
                .name("flow-roster-fetch".into())
                .spawn(move || {
                    let entry = fetch_roster_blocking(&cwd);
                    catalog.complete_refresh(cwd, entry);
                })
                .ok();
        }
    }

    /// Land a fetched roster: store the entry, bump the generation so
    /// main-menu caches invalidate on their next read, THEN fire the notify
    /// hook — a repaint triggered by the hook must already see the new
    /// generation, or the repaint reads the stale cache and the arrival is
    /// invisible until the next interaction.
    fn complete_refresh(&self, cwd: String, entry: RosterEntry) {
        self.entries.lock().insert(cwd, entry);
        ROSTER_GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.notify();
    }

    #[cfg(test)]
    fn insert_for_test(&self, cwd: &str, entry: RosterEntry) {
        self.entries.lock().insert(cwd.to_string(), entry);
    }

    #[cfg(test)]
    pub(crate) fn prime_ready_for_test(&self, cwd: &str) {
        self.insert_for_test(cwd, RosterEntry::empty(RosterStatus::Ready));
    }
}

/// One staleness decision for every cache-side read (`roster_for`, `poke`):
/// missing → fetch; stale → refetch unless a refresh is already in flight
/// (Loading). Keeping this shared means a hot grouped cache can never pin a
/// stale roster by reading through a path with laxer rules.
fn entry_needs_refresh(entry: Option<&RosterEntry>) -> bool {
    match entry {
        Some(entry) => entry.is_stale() && entry.status != RosterStatus::Loading,
        None => true,
    }
}

/// Run `<binary> roster --json` with a hard deadline. Stdout/stderr drain
/// on reader threads so a large roster can never deadlock the pipe while
/// the deadline loop polls `try_wait`.
fn run_roster_with_deadline(binary: &str, cwd: &str) -> std::io::Result<std::process::Output> {
    use std::io::Read;
    use std::process::Stdio;

    let mut child = Command::new(binary)
        .arg("roster")
        .arg("--json")
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    let stdout_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(pipe) = stdout_pipe.as_mut() {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(pipe) = stderr_pipe.as_mut() {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    });

    let started = Instant::now();
    let status = loop {
        match child.try_wait()? {
            Some(status) => break status,
            None if started.elapsed() > ROSTER_FETCH_DEADLINE => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!(
                        "roster fetch exceeded {}s deadline",
                        ROSTER_FETCH_DEADLINE.as_secs()
                    ),
                ));
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    };

    Ok(std::process::Output {
        status,
        stdout: stdout_reader.join().unwrap_or_default(),
        stderr: stderr_reader.join().unwrap_or_default(),
    })
}

/// Blocking roster fetch — call only from background threads.
pub fn fetch_roster_blocking(cwd: &str) -> RosterEntry {
    let Some(binary) = mdflow_binary() else {
        let mut entry = RosterEntry::empty(RosterStatus::Error);
        entry
            .warnings
            .push("mdflow CLI not found on PATH (npm i -g mdflow)".to_string());
        return entry;
    };
    if !Path::new(cwd).is_dir() {
        let mut entry = RosterEntry::empty(RosterStatus::Error);
        entry.warnings.push(format!("cwd does not exist: {cwd}"));
        return entry;
    }
    let output = match run_roster_with_deadline(binary, cwd) {
        Ok(output) => output,
        Err(err) => {
            let mut entry = RosterEntry::empty(RosterStatus::Error);
            entry
                .warnings
                .push(format!("failed to run {binary}: {err}"));
            return entry;
        }
    };
    if !output.status.success() {
        // Distinguish "this mdflow predates the protocol" from "this mdflow
        // supports roster but failed" — classifying every nonzero exit as
        // Legacy would hide real config/registry errors behind a calm
        // 'legacy mdflow' banner. Pre-protocol mdflow resolves `roster` as a
        // flow name and fails with a not-found error naming it.
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr_lower = stderr.to_lowercase();
        let looks_pre_protocol =
            stderr_lower.contains("not found") && stderr_lower.contains("roster");
        if looks_pre_protocol {
            return RosterEntry::empty(RosterStatus::Legacy);
        }
        let mut entry = RosterEntry::empty(RosterStatus::Error);
        let excerpt: String = stderr.trim().chars().take(300).collect();
        entry.warnings.push(if excerpt.is_empty() {
            format!("{binary} roster exited {}", output.status)
        } else {
            format!("{binary} roster exited {}: {excerpt}", output.status)
        });
        return entry;
    }
    parse_roster_output(&String::from_utf8_lossy(&output.stdout))
}

fn parse_roster_output(stdout: &str) -> RosterEntry {
    match serde_json::from_str::<RosterSnapshot>(stdout) {
        Ok(snapshot) if snapshot.protocol_version == FLOW_UX_PROTOCOL_VERSION => RosterEntry {
            status: RosterStatus::Ready,
            flows: Arc::new(snapshot.flows),
            warnings: snapshot.warnings,
            fetched_at: Instant::now(),
        },
        Ok(_) => RosterEntry::empty(RosterStatus::Legacy),
        Err(err) => {
            let mut entry = RosterEntry::empty(RosterStatus::Error);
            entry.warnings.push(format!("roster parse error: {err}"));
            entry
        }
    }
}

/// The full desk corpus for a cwd: `md roster` flows plus the installed
/// flows package. Package flows lose to a roster flow with the same name
/// (a project override of a packaged flow should win locally).
pub fn desk_flows(roster: &RosterEntry) -> Vec<FlowDescriptor> {
    let mut flows: Vec<FlowDescriptor> = roster.flows.iter().cloned().collect();
    let roster_names: std::collections::HashSet<&str> =
        roster.flows.iter().map(|f| f.name.as_str()).collect();
    for flow in crate::flows::package_source::package_flows() {
        if !roster_names.contains(flow.name.as_str()) {
            flows.push(flow);
        }
    }
    flows
}

/// Simple case-insensitive subsequence filter for roster rows, ranked:
/// name prefix > name contains > description contains. The friendly agent
/// name matches too, so "gmail" finds `flow-gmail` shown as "Gmail".
/// Frecency integration can replace this without touching renderers.
pub fn filter_flows<'a>(flows: &'a [FlowDescriptor], query: &str) -> Vec<&'a FlowDescriptor> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return flows.iter().collect();
    }
    let mut ranked: Vec<(u8, &FlowDescriptor)> = flows
        .iter()
        .filter_map(|flow| {
            let name = flow.name.to_lowercase();
            let friendly = flow.friendly_name().to_lowercase();
            let description = flow
                .description
                .as_deref()
                .unwrap_or_default()
                .to_lowercase();
            if name.starts_with(&query) || friendly.starts_with(&query) {
                Some((0u8, flow))
            } else if name.contains(&query) || friendly.contains(&query) {
                Some((1u8, flow))
            } else if description.contains(&query) {
                Some((2u8, flow))
            } else {
                None
            }
        })
        .collect();
    ranked.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.name.cmp(&b.1.name)));
    ranked.into_iter().map(|(_, flow)| flow).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flows::model::FlowSource;

    fn descriptor(name: &str, description: Option<&str>) -> FlowDescriptor {
        serde_json::from_value(serde_json::json!({
            "id": format!("project:{name}"),
            "path": format!("/tmp/p/flows/{name}.md"),
            "source": "project",
            "name": name,
            "description": description,
            "engine": "pi",
            "inputs": [],
            "isWorkflow": false,
            "interactive": false,
            "mtimeMs": 0
        }))
        .expect("descriptor builds")
    }

    #[test]
    fn parse_roster_output_accepts_protocol_v1() {
        let entry = parse_roster_output(
            r#"{"protocolVersion":1,"cwd":"/p","projectRoot":"/p","flows":[],"warnings":["w"]}"#,
        );
        assert_eq!(entry.status, RosterStatus::Ready);
        assert_eq!(entry.warnings, vec!["w".to_string()]);
    }

    #[test]
    fn parse_roster_output_flags_future_protocol_as_legacy() {
        let entry =
            parse_roster_output(r#"{"protocolVersion":2,"cwd":"/p","flows":[],"warnings":[]}"#);
        assert_eq!(entry.status, RosterStatus::Legacy);
    }

    #[test]
    fn parse_roster_output_reports_garbage_as_error() {
        let entry = parse_roster_output("not json");
        assert_eq!(entry.status, RosterStatus::Error);
        assert!(!entry.warnings.is_empty());
    }

    #[test]
    fn filter_ranks_prefix_over_contains_over_description() {
        let flows = vec![
            descriptor("deploy", Some("ship it")),
            descriptor("redeploy", None),
            descriptor("notes", Some("deploy notes helper")),
        ];
        let hits = filter_flows(&flows, "dep");
        let names: Vec<&str> = hits.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["deploy", "redeploy", "notes"]);
        assert_eq!(flows[0].source, FlowSource::Project);
    }

    #[test]
    fn empty_query_returns_all_in_roster_order() {
        let flows = vec![descriptor("b", None), descriptor("a", None)];
        let hits = filter_flows(&flows, "  ");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].name, "b", "roster order preserved for empty query");
    }

    #[test]
    fn refresh_decision_fetches_missing_and_stale_but_not_inflight() {
        assert!(entry_needs_refresh(None), "missing cwd must fetch");

        let fresh = RosterEntry::empty(RosterStatus::Ready);
        assert!(
            !entry_needs_refresh(Some(&fresh)),
            "fresh entry must not refetch"
        );

        let mut stale = RosterEntry::empty(RosterStatus::Ready);
        stale.fetched_at = Instant::now()
            .checked_sub(ROSTER_TTL + Duration::from_secs(1))
            .unwrap_or_else(Instant::now);
        if stale.is_stale() {
            assert!(
                entry_needs_refresh(Some(&stale)),
                "stale entry must refetch"
            );
            stale.status = RosterStatus::Loading;
            assert!(
                !entry_needs_refresh(Some(&stale)),
                "in-flight refresh must not stack another"
            );
        }
    }

    #[test]
    fn completed_refresh_bumps_generation_before_notifying() {
        let catalog = FlowCatalog::default();
        let seen_generation = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let hook_seen = Arc::clone(&seen_generation);
        catalog.set_notify_hook(move || {
            hook_seen.store(roster_generation(), std::sync::atomic::Ordering::SeqCst);
        });
        let before = roster_generation();
        catalog.complete_refresh(
            "/tmp/gen-cwd".to_string(),
            RosterEntry::empty(RosterStatus::Ready),
        );
        let after = roster_generation();
        assert!(after > before, "landing a roster must bump the generation");
        assert_eq!(
            seen_generation.load(std::sync::atomic::Ordering::SeqCst),
            after,
            "notify hook must observe the NEW generation (bump-then-notify)"
        );
        assert!(catalog.entries.lock().contains_key("/tmp/gen-cwd"));
    }

    #[test]
    fn catalog_returns_cached_entry_without_blocking() {
        let catalog = FlowCatalog::default();
        let mut entry = RosterEntry::empty(RosterStatus::Ready);
        entry.flows = Arc::new(vec![descriptor("cached", None)]);
        catalog.insert_for_test("/tmp/cwd", entry);
        let got = catalog.entries.lock().get("/tmp/cwd").cloned().unwrap();
        assert_eq!(got.flows.len(), 1);
    }
}
