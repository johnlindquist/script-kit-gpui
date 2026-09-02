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
    /// Non-fatal roster warnings supplied by mdflow. These are diagnostic
    /// context, not primary UI copy or automation payload.
    pub warnings: Vec<String>,
    /// Typed failure for a failed roster fetch. Raw stderr/parse detail lives
    /// only in its diagnostic vault descriptor.
    pub failure: Option<crate::ai::reliability::AppFailureRecord>,
    pub fetched_at: Instant,
}

impl RosterEntry {
    fn empty(status: RosterStatus) -> Self {
        Self {
            status,
            flows: Arc::new(Vec::new()),
            warnings: Vec::new(),
            failure: None,
            fetched_at: Instant::now(),
        }
    }

    fn failed(failure: crate::ai::reliability::AppFailureRecord) -> Self {
        Self {
            failure: Some(failure),
            ..Self::empty(RosterStatus::Error)
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
    if crate::runtime_policy::is_owned_evaluation() {
        return None;
    }
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

struct CachedRosterEntry {
    entry: RosterEntry,
    generation: u64,
}

type NotifyHook = Box<dyn Fn(&str, u64) + Send + Sync>;

#[derive(Default)]
pub struct FlowCatalog {
    entries: Mutex<HashMap<String, CachedRosterEntry>>,
    /// One worker and one latest source-change request per working directory.
    refreshing: Mutex<HashMap<String, bool>>,
    notify: Mutex<Option<NotifyHook>>,
}

impl FlowCatalog {
    pub fn install_owned_roster(
        &self,
        cwd: String,
        flows: Vec<FlowDescriptor>,
    ) -> anyhow::Result<()> {
        let scope = crate::runtime_policy::owned_evaluation()
            .ok_or_else(|| anyhow::anyhow!("owned_flow_required"))?;
        scope.require_owned_path(std::path::Path::new(&cwd))?;
        anyhow::ensure!(flows.len() <= 32, "flow_fixture_limit");
        self.complete_refresh(
            cwd,
            RosterEntry {
                status: RosterStatus::Ready,
                flows: Arc::new(flows),
                warnings: Vec::new(),
                failure: None,
                fetched_at: Instant::now(),
            },
        );
        Ok(())
    }

    pub fn set_notify_hook(&self, hook: impl Fn(&str, u64) + Send + Sync + 'static) {
        *self.notify.lock() = Some(Box::new(hook));
    }

    fn notify(&self, cwd: &str, generation: u64) {
        if let Some(hook) = self.notify.lock().as_ref() {
            hook(cwd, generation);
        }
    }

    pub fn roster_generation_for(&self, cwd: &str) -> u64 {
        self.entries
            .lock()
            .get(cwd)
            .map_or(0, |cached| cached.generation)
    }

    /// Current entry for a cwd without blocking; kicks off a background
    /// refresh when missing or stale. Renderers call this every frame.
    pub fn roster_for(self: &Arc<Self>, cwd: &str) -> RosterEntry {
        if crate::runtime_policy::is_owned_evaluation() {
            return self
                .entries
                .lock()
                .get(cwd)
                .map(|cached| cached.entry.clone())
                .unwrap_or_else(|| RosterEntry::empty(RosterStatus::Ready));
        }
        let needs_refresh = {
            let entries = self.entries.lock();
            entry_needs_refresh(entries.get(cwd).map(|cached| &cached.entry))
        };
        if needs_refresh {
            self.spawn_refresh(cwd, false);
        }
        self.entries
            .lock()
            .get(cwd)
            .map(|cached| cached.entry.clone())
            .unwrap_or_else(|| RosterEntry::empty(RosterStatus::Loading))
    }

    /// Force refresh (cwd chip changed, manual reload action).
    pub fn refresh(self: &Arc<Self>, cwd: &str) {
        self.spawn_refresh(cwd, true);
    }

    /// Cheap staleness check without cloning the entry: spawns a background
    /// refresh when the cwd's roster is stale or missing. Main-menu cache
    /// getters call this every read so a hot cache can never pin a stale
    /// roster forever (the refresh completion bumps the generation, which
    /// invalidates those caches and repaints via the notify hook).
    pub fn poke(self: &Arc<Self>, cwd: &str) {
        let needs_refresh = {
            let entries = self.entries.lock();
            entry_needs_refresh(entries.get(cwd).map(|cached| &cached.entry))
        };
        if needs_refresh {
            self.spawn_refresh(cwd, false);
        }
    }

    fn spawn_refresh(self: &Arc<Self>, cwd: &str, source_changed: bool) {
        if crate::runtime_policy::is_owned_evaluation() {
            return;
        }
        {
            let mut refreshing = self.refreshing.lock();
            if let Some(desired) = refreshing.get_mut(cwd) {
                *desired |= source_changed;
                return;
            }
            refreshing.insert(cwd.to_string(), false);
            self.entries
                .lock()
                .entry(cwd.to_string())
                .or_insert_with(|| CachedRosterEntry {
                    entry: RosterEntry::empty(RosterStatus::Loading),
                    generation: 0,
                })
                .entry
                .status = RosterStatus::Loading;
        }

        // GPUI tests keep discovery synchronous under the deterministic scheduler.
        #[cfg(test)]
        {
            let mut refreshing = self.refreshing.lock();
            let generation = self.store_refresh(cwd, RosterEntry::empty(RosterStatus::Ready));
            refreshing.remove(cwd);
            drop(refreshing);
            self.notify(cwd, generation);
            return;
        }

        #[cfg(not(test))]
        {
            let catalog = Arc::clone(self);
            let work_cwd = cwd.to_string();
            let spawned = std::thread::Builder::new()
                .name("flow-roster-fetch".into())
                .spawn(move || loop {
                    let entry = fetch_roster_blocking(&work_cwd);
                    let mut refreshing = catalog.refreshing.lock();
                    if refreshing.get_mut(&work_cwd).is_some_and(std::mem::take) {
                        // A newer source request supersedes this snapshot.
                        // Re-read on the same worker; never stack another fetch.
                        drop(refreshing);
                        continue;
                    }
                    let generation = catalog.store_refresh(&work_cwd, entry);
                    refreshing.remove(&work_cwd);
                    drop(refreshing);
                    catalog.notify(&work_cwd, generation);
                    break;
                });
            if let Err(error) = spawned {
                let entry =
                    RosterEntry::failed(crate::ai::reliability::process_failure_with_detail(
                        sk_protocol::ai_reliability::ProtocolComponent::Mdflow,
                        crate::ai::reliability::ProcessFailureFacts::SpawnFailed,
                        &format!("flow roster worker could not start: {error}"),
                    ));
                let mut refreshing = self.refreshing.lock();
                let generation = self.store_refresh(cwd, entry);
                refreshing.remove(cwd);
                drop(refreshing);
                self.notify(cwd, generation);
            }
        }
    }

    /// Land a fetched roster: store the entry, bump the generation so
    /// main-menu caches invalidate on their next read, THEN fire the notify
    /// hook — a repaint triggered by the hook must already see the new
    /// generation, or the repaint reads the stale cache and the arrival is
    /// invisible until the next interaction.
    fn complete_refresh(&self, cwd: String, entry: RosterEntry) {
        let generation = self.store_refresh(&cwd, entry);
        self.notify(&cwd, generation);
    }

    #[expect(
        clippy::expect_used,
        reason = "Generation exhaustion must stop publication rather than reuse a roster identity."
    )]
    fn store_refresh(&self, cwd: &str, mut entry: RosterEntry) -> u64 {
        let mut entries = self.entries.lock();
        if matches!(entry.status, RosterStatus::Error | RosterStatus::Legacy) {
            if let Some(previous) = entries.get(cwd) {
                entry.flows = previous.entry.flows.clone();
            }
        }
        let generation = ROSTER_GENERATION
            .fetch_update(
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
                |generation| generation.checked_add(1),
            )
            .expect("flow catalogue generation exhausted")
            + 1;
        let cached = CachedRosterEntry { entry, generation };
        if let Some(previous) = entries.get_mut(cwd) {
            *previous = cached;
        } else {
            entries.insert(cwd.to_owned(), cached);
        }
        generation
    }

    #[cfg(test)]
    fn insert_for_test(&self, cwd: &str, entry: RosterEntry) {
        self.entries.lock().insert(
            cwd.to_string(),
            CachedRosterEntry {
                entry,
                generation: 0,
            },
        );
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
        return RosterEntry::failed(crate::ai::reliability::process_failure_with_detail(
            sk_protocol::ai_reliability::ProtocolComponent::Mdflow,
            crate::ai::reliability::ProcessFailureFacts::SpawnFailed,
            "mdflow CLI not found on PATH",
        ));
    };
    if !Path::new(cwd).is_dir() {
        return RosterEntry::failed(crate::ai::reliability::process_failure_with_detail(
            sk_protocol::ai_reliability::ProtocolComponent::Mdflow,
            crate::ai::reliability::ProcessFailureFacts::SpawnFailed,
            &format!("cwd does not exist: {cwd}"),
        ));
    }
    let output = match run_roster_with_deadline(binary, cwd) {
        Ok(output) => output,
        Err(err) => {
            return RosterEntry::failed(crate::ai::reliability::process_failure_with_detail(
                sk_protocol::ai_reliability::ProtocolComponent::Mdflow,
                crate::ai::reliability::ProcessFailureFacts::SpawnFailed,
                &format!("failed to run {binary}: {err}"),
            ));
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
        let detail = if stderr.trim().is_empty() {
            format!("{binary} roster exited {}", output.status)
        } else {
            format!("{binary} roster exited {}: {stderr}", output.status)
        };
        return RosterEntry::failed(crate::ai::reliability::process_failure_with_detail(
            sk_protocol::ai_reliability::ProtocolComponent::Mdflow,
            crate::ai::reliability::ProcessFailureFacts::ChildExited {
                exit_code: output.status.code(),
                signal: None,
            },
            &detail,
        ));
    }
    parse_roster_output(&String::from_utf8_lossy(&output.stdout))
}

fn parse_roster_output(stdout: &str) -> RosterEntry {
    match serde_json::from_str::<RosterSnapshot>(stdout) {
        Ok(snapshot) if snapshot.protocol_version == FLOW_UX_PROTOCOL_VERSION => RosterEntry {
            status: RosterStatus::Ready,
            flows: Arc::new(snapshot.flows),
            warnings: snapshot.warnings,
            failure: None,
            fetched_at: Instant::now(),
        },
        Ok(_) => RosterEntry::empty(RosterStatus::Legacy),
        Err(err) => RosterEntry::failed(crate::ai::reliability::protocol_failure_with_detail(
            sk_protocol::ai_reliability::ProtocolComponent::Mdflow,
            crate::ai::reliability::ProtocolFailureFacts::MalformedResponse,
            &format!("roster parse error: {err}; payload: {stdout}"),
        )),
    }
}

/// The full desk corpus for a cwd: `md roster` flows plus the installed
/// flows package. Package flows lose to a roster flow with the same name
/// (a project override of a packaged flow should win locally).
pub fn desk_flows(roster: &RosterEntry) -> Vec<FlowDescriptor> {
    let mut flows: Vec<FlowDescriptor> = roster.flows.iter().cloned().collect();
    if crate::runtime_policy::is_owned_evaluation() {
        return flows;
    }
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
        assert!(
            entry.warnings.is_empty(),
            "raw parse text is diagnostic-only"
        );
        assert!(entry.failure.is_some());
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
        catalog.set_notify_hook(move |cwd, generation| {
            assert_eq!(cwd, "/tmp/gen-cwd");
            assert!(roster_generation() >= generation);
            hook_seen.store(generation, std::sync::atomic::Ordering::SeqCst);
        });
        let before = roster_generation();
        catalog.complete_refresh(
            "/tmp/gen-cwd".to_string(),
            RosterEntry::empty(RosterStatus::Ready),
        );
        let after = catalog.roster_generation_for("/tmp/gen-cwd");
        assert!(after > before, "landing a roster must bump the generation");
        assert_eq!(
            seen_generation.load(std::sync::atomic::Ordering::SeqCst),
            after,
            "notify hook must observe the NEW generation (bump-then-notify)"
        );
        assert!(catalog.entries.lock().contains_key("/tmp/gen-cwd"));
    }

    #[test]
    fn unrelated_cwd_completion_keeps_the_current_cwd_revision_unchanged() {
        let catalog = FlowCatalog::default();
        let notifications = Arc::new(Mutex::new(Vec::new()));
        let received = notifications.clone();
        catalog.set_notify_hook(move |cwd, generation| {
            received.lock().push((cwd.to_owned(), generation));
        });
        catalog.complete_refresh(
            "/tmp/current-cwd".into(),
            RosterEntry::empty(RosterStatus::Ready),
        );
        let current_generation = catalog.roster_generation_for("/tmp/current-cwd");
        catalog.complete_refresh(
            "/tmp/other-cwd".into(),
            RosterEntry::empty(RosterStatus::Ready),
        );
        let other_generation = catalog.roster_generation_for("/tmp/other-cwd");
        assert_eq!(
            catalog.roster_generation_for("/tmp/current-cwd"),
            current_generation
        );
        assert!(other_generation > current_generation);
        assert_eq!(
            *notifications.lock(),
            vec![
                ("/tmp/current-cwd".to_owned(), current_generation),
                ("/tmp/other-cwd".to_owned(), other_generation),
            ]
        );
    }

    #[test]
    fn failed_or_unavailable_refresh_retains_last_good_but_successful_empty_clears() {
        let catalog = FlowCatalog::default();
        let mut ready = RosterEntry::empty(RosterStatus::Ready);
        ready.flows = Arc::new(vec![descriptor("last-good", None)]);
        catalog.complete_refresh("/tmp/retained-cwd".into(), ready);
        for status in [RosterStatus::Error, RosterStatus::Legacy] {
            catalog.complete_refresh("/tmp/retained-cwd".into(), RosterEntry::empty(status));
            let entries = catalog.entries.lock();
            let entry = &entries["/tmp/retained-cwd"].entry;
            assert_eq!(entry.status, status);
            assert_eq!(entry.flows.len(), 1);
            assert_eq!(entry.flows[0].name, "last-good");
        }
        catalog.complete_refresh(
            "/tmp/retained-cwd".into(),
            RosterEntry::empty(RosterStatus::Ready),
        );
        assert!(catalog.entries.lock()["/tmp/retained-cwd"]
            .entry
            .flows
            .is_empty());
    }

    #[test]
    fn catalog_returns_cached_entry_without_blocking() {
        let catalog = FlowCatalog::default();
        let mut entry = RosterEntry::empty(RosterStatus::Ready);
        entry.flows = Arc::new(vec![descriptor("cached", None)]);
        catalog.insert_for_test("/tmp/cwd", entry);
        let got = catalog
            .entries
            .lock()
            .get("/tmp/cwd")
            .map(|cached| cached.entry.clone())
            .unwrap();
        assert_eq!(got.flows.len(), 1);
    }
}
