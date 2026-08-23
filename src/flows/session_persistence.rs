// ---------------------------------------------------------------------
// Conversation persistence (survives app restarts)
// ---------------------------------------------------------------------

/// One most-recent conversation snapshot per flow, rewritten after every
/// committed turn. `flow_sessions` is in-memory only, so a dev rebuild or
/// app restart used to strand the user's conversation: Enter on the flow's
/// launcher row landed in a blank composer (2026-07-10 report). A restored
/// session sets `needs_rethread`, so the next submit rolls this transcript
/// back into the engine prompt via `build_turn_task`.
///
/// Identity is `flow_id` + `flow_path`: protocol flow ids are only
/// `<source>:<slug>` (`project:review`), so two different projects can carry
/// the same id — keying by id alone restored the WRONG project's transcript
/// into the wrong agent (2026-07-11 audit P0, correctness + privacy).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PersistedFlowConversation {
    pub flow_id: String,
    /// Definition path this conversation belongs to (empty on legacy
    /// snapshots persisted before identity was path-qualified).
    #[serde(default)]
    pub flow_path: String,
    pub saved_at: String,
    /// Snapshot format version. 0 (absent) = legacy: either two-field turns
    /// or transitional records whose Stopped assistants carry the UI caption
    /// baked into the text. 2 = raw assistant text with the caption derived
    /// from `outcome` at display time, failures as raw caption strings.
    /// `SNAPSHOT_VERSION` (3) = failures persisted as typed
    /// [`PersistedAiFailure`] records; the legacy `error` field is never
    /// written and is classified into a typed record while loading.
    #[serde(default)]
    pub version: u32,
    /// Monotonic model revision. Version-4 writers start at one; the store uses
    /// it with per-thread tombstones to reject stale asynchronous snapshots.
    #[serde(default)]
    pub revision: u64,
    /// Version-4 active thread identity. Empty on legacy v0-v3 snapshots.
    #[serde(default)]
    pub active_thread_id: String,
    /// Version-4 thread manifest. Legacy snapshots migrate their `turns` into
    /// one active thread without dropping any rows.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub threads: Vec<PersistedFlowThread>,
    /// Legacy v0-v3 turn vector. Read-only compatibility; v4 never writes it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub turns: Vec<PersistedFlowTurn>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PersistedFlowThreadState {
    Active,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PersistedFlowThread {
    pub id: String,
    pub state: PersistedFlowThreadState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_thread_id: Option<String>,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<String>,
    #[serde(default)]
    pub inherited_turn_count: usize,
    pub turns: Vec<PersistedFlowTurn>,
}

/// Current snapshot format: a v4 active-thread manifest plus immutable
/// archives. Turns retain raw assistant text, structured outcome, and typed
/// persisted failures.
pub const SNAPSHOT_VERSION: u32 = 4;

/// Convert a persisted snapshot into the ONE canonical in-memory turn vector
/// (Oracle 2026-07-21, WP-A4): restore must render and store from this same
/// vector, never from the raw persisted fields.
///
/// Normalization invariants:
/// - `Ok`/`Stopped` ⇒ `failure = None` (stopped turns never carry a failure).
/// - `Failed` ⇒ `failure = Some(typed)`: the typed record when the snapshot
///   has one, otherwise the legacy v0–v2 `error` caption classified while
///   loading (blank/absent → the `Unknown` default).
/// - Pre-version-2 Stopped records may carry the UI caption baked into the
///   assistant text; strip exactly one canonical caption suffix so
///   `assistant` is raw engine output.
pub fn canonical_session_turns(snapshot: &PersistedFlowConversation) -> Vec<SessionTurn> {
    let persisted_turns = if snapshot.version >= 4 {
        snapshot
            .threads
            .iter()
            .find(|thread| {
                thread.id == snapshot.active_thread_id
                    && thread.state == PersistedFlowThreadState::Active
            })
            .map(|thread| thread.turns.as_slice())
            .unwrap_or(&[])
    } else {
        snapshot.turns.as_slice()
    };
    canonical_persisted_turns(snapshot.version, persisted_turns)
}

pub fn canonical_persisted_turns(
    version: u32,
    persisted_turns: &[PersistedFlowTurn],
) -> Vec<SessionTurn> {
    const CAPTION_DERIVED_VERSION: u32 = 2;
    persisted_turns
        .iter()
        .map(|turn| {
            let mut assistant = turn.assistant.clone();
            if version < CAPTION_DERIVED_VERSION && turn.outcome == PersistedTurnOutcome::Stopped {
                if assistant == FLOW_STOPPED_CAPTION {
                    assistant.clear();
                } else if let Some(stripped) =
                    assistant.strip_suffix(&format!("\n\n{FLOW_STOPPED_CAPTION}"))
                {
                    assistant = stripped.to_string();
                }
            }
            let failure = match turn.outcome {
                PersistedTurnOutcome::Ok | PersistedTurnOutcome::Stopped => None,
                PersistedTurnOutcome::Failed => Some(match &turn.failure {
                    Some(failure) => failure.clone(),
                    None => turn
                        .error
                        .as_deref()
                        .map(str::trim)
                        .filter(|error| !error.is_empty())
                        .map(PersistedAiFailure::from_legacy_error)
                        .unwrap_or_else(PersistedAiFailure::unknown_default),
                }),
            };
            SessionTurn {
                user: turn.user.clone(),
                assistant,
                outcome: turn.outcome,
                failure,
            }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PersistedFlowTurn {
    pub user: String,
    pub assistant: String,
    #[serde(default)]
    pub outcome: PersistedTurnOutcome,
    /// Legacy (v0–v2) raw failure caption. Read-only: version-3 snapshots
    /// never write it; loading classifies it into `failure`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Typed persisted failure (version 3+).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<PersistedAiFailure>,
}

impl From<&SessionTurn> for PersistedFlowTurn {
    fn from(turn: &SessionTurn) -> Self {
        Self {
            user: turn.user.clone(),
            assistant: turn.assistant.clone(),
            outcome: turn.outcome,
            error: None,
            failure: turn.failure.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PersistedTurnOutcome {
    #[default]
    Ok,
    Stopped,
    Failed,
}

fn conversation_store_dir() -> std::path::PathBuf {
    crate::setup::get_kit_path()
        .join("flows")
        .join("conversations")
}

/// Filesystem-safe slug of one identity component. Output is pure ASCII, so
/// byte-slicing the result is always char-boundary-safe.
fn sanitize_component(raw: &str) -> String {
    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// Previous path-qualified names were lossy: punctuation collapsed to `-`
/// and long paths discarded their prefixes. Keep the exact old spelling only
/// for owner-verified, one-shot migration into the digest-qualified store.
fn legacy_path_qualified_conversation_file_name(flow_id: &str, flow_path: &str) -> String {
    let id = sanitize_component(flow_id);
    let mut path = sanitize_component(flow_path.trim_start_matches('/'));
    const PATH_PORTION_MAX: usize = 160;
    if path.len() > PATH_PORTION_MAX {
        path = path[path.len() - PATH_PORTION_MAX..].to_string();
    }
    format!("{id}--{path}.json")
}

/// The readable slug is diagnostic only; a length-framed SHA-256 of BOTH
/// original identity components prevents punctuation, delimiter, Unicode,
/// and truncated-path collisions from sharing a private conversation.
fn conversation_file_name(flow_id: &str, flow_path: &str) -> String {
    use sha2::Digest;

    let mut digest = sha2::Sha256::new();
    digest.update(b"script-kit-flow-conversation-v1\0");
    for component in [flow_id, flow_path] {
        digest.update((component.len() as u64).to_be_bytes());
        digest.update(component.as_bytes());
    }

    let mut id = sanitize_component(flow_id);
    const ID_PORTION_MAX: usize = 60;
    if id.len() > ID_PORTION_MAX {
        id.truncate(ID_PORTION_MAX);
    }
    let mut path = sanitize_component(flow_path.trim_start_matches('/'));
    const PATH_PORTION_MAX: usize = 120;
    if path.len() > PATH_PORTION_MAX {
        path = path[path.len() - PATH_PORTION_MAX..].to_string();
    }

    format!("{id}--{path}--{:x}.json", digest.finalize())
}

/// Legacy (pre path-qualified identity) file name, keyed by flow id alone.
fn legacy_conversation_file_name(flow_id: &str) -> String {
    format!("{}.json", sanitize_component(flow_id))
}

fn migrated_thread_id(flow_id: &str, flow_path: &str) -> String {
    format!(
        "flow-thread-migrated-{}",
        crate::ai::reliability::redacted_fingerprint(&format!("{flow_id}:{flow_path}"))
    )
}

fn snapshot_from_turns(
    flow_id: &str,
    flow_path: &str,
    turns: &[SessionTurn],
) -> PersistedFlowConversation {
    let thread_id = migrated_thread_id(flow_id, flow_path);
    let now = chrono::Utc::now().to_rfc3339();
    PersistedFlowConversation {
        flow_id: flow_id.to_string(),
        flow_path: flow_path.to_string(),
        saved_at: now.clone(),
        version: SNAPSHOT_VERSION,
        revision: 1,
        active_thread_id: thread_id.clone(),
        threads: vec![PersistedFlowThread {
            id: thread_id,
            state: PersistedFlowThreadState::Active,
            parent_thread_id: None,
            created_at: now,
            archived_at: None,
            inherited_turn_count: 0,
            turns: turns.iter().map(PersistedFlowTurn::from).collect(),
        }],
        turns: Vec::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersistedConversationLoadError {
    FutureVersion(u32),
    IdentityMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalizedConversation {
    pub snapshot: PersistedFlowConversation,
    pub changed: bool,
}

fn canonical_timestamp(value: &str, fallback: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|timestamp| {
            timestamp
                .with_timezone(&chrono::Utc)
                .to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)
        })
        .unwrap_or_else(|_| fallback.to_string())
}

fn recovered_thread_id(flow_id: &str, flow_path: &str, index: usize, raw_id: &str) -> String {
    format!(
        "flow-thread-recovered-{}",
        crate::ai::reliability::redacted_fingerprint(&format!(
            "{flow_id}:{flow_path}:{index}:{raw_id}"
        ))
    )
}

fn remove_retained_parent_cycles(threads: &mut [PersistedFlowThread]) {
    let parent_by_id: std::collections::HashMap<String, Option<String>> = threads
        .iter()
        .map(|thread| (thread.id.clone(), thread.parent_thread_id.clone()))
        .collect();
    for thread in threads.iter_mut() {
        let start = thread.id.clone();
        let mut cursor = thread.parent_thread_id.clone();
        let mut visited = std::collections::HashSet::new();
        while let Some(parent) = cursor {
            if parent == start || !visited.insert(parent.clone()) {
                thread.parent_thread_id = None;
                break;
            }
            cursor = parent_by_id.get(&parent).cloned().flatten();
        }
    }
}

/// Normalize any persisted conversation into the single canonical v4 shape.
/// The caller captures `now` once so repairs never depend on repeated clock
/// reads. Future versions fail closed and are never rewritten.
pub fn canonicalize_persisted_conversation(
    raw: PersistedFlowConversation,
    expected_flow_id: &str,
    expected_flow_path: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<CanonicalizedConversation, PersistedConversationLoadError> {
    if raw.version > SNAPSHOT_VERSION {
        return Err(PersistedConversationLoadError::FutureVersion(raw.version));
    }
    if raw.flow_id != expected_flow_id
        || (raw.flow_path != expected_flow_path
            && !(raw.version < SNAPSHOT_VERSION && raw.flow_path.is_empty()))
    {
        return Err(PersistedConversationLoadError::IdentityMismatch);
    }

    let original = raw.clone();
    let now = now.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true);
    let saved_at = canonical_timestamp(&raw.saved_at, &now);

    if raw.version < SNAPSHOT_VERSION {
        let turns = canonical_persisted_turns(raw.version, &raw.turns);
        let thread_id = migrated_thread_id(expected_flow_id, expected_flow_path);
        let snapshot = PersistedFlowConversation {
            flow_id: expected_flow_id.to_string(),
            flow_path: expected_flow_path.to_string(),
            saved_at: saved_at.clone(),
            version: SNAPSHOT_VERSION,
            revision: 1,
            active_thread_id: thread_id.clone(),
            threads: vec![PersistedFlowThread {
                id: thread_id,
                state: PersistedFlowThreadState::Active,
                parent_thread_id: None,
                created_at: saved_at,
                archived_at: None,
                inherited_turn_count: 0,
                turns: turns.iter().map(PersistedFlowTurn::from).collect(),
            }],
            turns: Vec::new(),
        };
        return Ok(CanonicalizedConversation {
            changed: snapshot != original,
            snapshot,
        });
    }

    let raw_threads = raw.threads;
    let active_index = if !raw.active_thread_id.is_empty() {
        let matches: Vec<usize> = raw_threads
            .iter()
            .enumerate()
            .filter_map(|(index, thread)| (thread.id == raw.active_thread_id).then_some(index))
            .collect();
        (matches.len() == 1).then_some(matches[0])
    } else {
        None
    }
    .or_else(|| {
        let active: Vec<usize> = raw_threads
            .iter()
            .enumerate()
            .filter_map(|(index, thread)| {
                (thread.state == PersistedFlowThreadState::Active).then_some(index)
            })
            .collect();
        match active.as_slice() {
            [] => None,
            [only] => Some(*only),
            many => many.last().copied(),
        }
    })
    .or_else(|| raw_threads.len().checked_sub(1));

    let mut id_counts = std::collections::HashMap::<String, usize>::new();
    for thread in &raw_threads {
        *id_counts.entry(thread.id.clone()).or_default() += 1;
    }
    let mut used_ids = std::collections::HashSet::new();
    let canonical_ids: Vec<String> = raw_threads
        .iter()
        .enumerate()
        .map(|(index, thread)| {
            if !thread.id.is_empty() && used_ids.insert(thread.id.clone()) {
                thread.id.clone()
            } else {
                let mut recovered =
                    recovered_thread_id(expected_flow_id, expected_flow_path, index, &thread.id);
                while !used_ids.insert(recovered.clone()) {
                    recovered.push('x');
                }
                recovered
            }
        })
        .collect();

    let mut threads = Vec::with_capacity(raw_threads.len().max(1));
    for (index, thread) in raw_threads.into_iter().enumerate() {
        let is_active = Some(index) == active_index;
        let created_at = canonical_timestamp(&thread.created_at, &saved_at);
        let id = canonical_ids[index].clone();
        let parent_thread_id = thread.parent_thread_id.and_then(|parent| {
            if parent == thread.id || parent == id {
                return None;
            }
            match id_counts.get(&parent).copied() {
                Some(1) => original
                    .threads
                    .iter()
                    .position(|candidate| candidate.id == parent)
                    .map(|parent_index| canonical_ids[parent_index].clone()),
                Some(_) => None,
                None if parent.is_empty() => None,
                None => Some(parent),
            }
        });
        let archived_at = if is_active {
            None
        } else {
            let candidate = thread
                .archived_at
                .as_deref()
                .map(|value| canonical_timestamp(value, &saved_at))
                .unwrap_or_else(|| saved_at.clone());
            let created = chrono::DateTime::parse_from_rfc3339(&created_at).ok();
            let archived = chrono::DateTime::parse_from_rfc3339(&candidate).ok();
            Some(
                if created
                    .zip(archived)
                    .is_some_and(|(created, archived)| archived >= created)
                {
                    candidate
                } else {
                    created_at.clone()
                },
            )
        };
        let turns = canonical_persisted_turns(SNAPSHOT_VERSION, &thread.turns);
        threads.push(PersistedFlowThread {
            id,
            state: if is_active {
                PersistedFlowThreadState::Active
            } else {
                PersistedFlowThreadState::Archived
            },
            parent_thread_id,
            created_at,
            archived_at,
            inherited_turn_count: thread.inherited_turn_count.min(turns.len()),
            turns: turns.iter().map(PersistedFlowTurn::from).collect(),
        });
    }

    let active_thread_id = if let Some(active_position) = threads
        .iter()
        .position(|thread| thread.state == PersistedFlowThreadState::Active)
    {
        let active = threads.remove(active_position);
        let active_thread_id = active.id.clone();
        threads.push(active);
        active_thread_id
    } else {
        let thread_id = migrated_thread_id(expected_flow_id, expected_flow_path);
        let turns = canonical_persisted_turns(SNAPSHOT_VERSION, &raw.turns);
        threads.push(PersistedFlowThread {
            id: thread_id.clone(),
            state: PersistedFlowThreadState::Active,
            parent_thread_id: None,
            created_at: saved_at.clone(),
            archived_at: None,
            inherited_turn_count: 0,
            turns: turns.iter().map(PersistedFlowTurn::from).collect(),
        });
        thread_id
    };

    remove_retained_parent_cycles(&mut threads);
    let snapshot = PersistedFlowConversation {
        flow_id: expected_flow_id.to_string(),
        flow_path: expected_flow_path.to_string(),
        saved_at,
        version: SNAPSHOT_VERSION,
        revision: raw.revision.max(1),
        active_thread_id,
        threads,
        turns: Vec::new(),
    };
    Ok(CanonicalizedConversation {
        changed: snapshot != original,
        snapshot,
    })
}

pub fn persist_conversation_snapshot_to(
    dir: &std::path::Path,
    snapshot: &PersistedFlowConversation,
) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(conversation_file_name(
        &snapshot.flow_id,
        &snapshot.flow_path,
    ));
    let bytes = serde_json::to_vec_pretty(snapshot).map_err(std::io::Error::other)?;
    crate::atomic_file::write_private_atomic(&path, &bytes)
}

pub fn persist_conversation_to(
    dir: &std::path::Path,
    flow_id: &str,
    flow_path: &str,
    turns: &[SessionTurn],
) -> std::io::Result<()> {
    persist_conversation_snapshot_to(dir, &snapshot_from_turns(flow_id, flow_path, turns))
}

pub fn load_persisted_conversation_from(
    dir: &std::path::Path,
    flow_id: &str,
    flow_path: &str,
) -> Option<PersistedFlowConversation> {
    let path = dir.join(conversation_file_name(flow_id, flow_path));
    match crate::atomic_file::read_private_file(&path) {
        Ok(raw) => {
            let snapshot: PersistedFlowConversation = serde_json::from_str(&raw).ok()?;
            let canonical = canonicalize_persisted_conversation(
                snapshot,
                flow_id,
                flow_path,
                chrono::Utc::now(),
            )
            .ok()?;
            if canonical.changed {
                let _ = persist_conversation_snapshot_to(dir, &canonical.snapshot);
            }
            return Some(canonical.snapshot);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return None,
    }
    // Existing path-qualified files predate the collision-resistant digest.
    // Their embedded owner must match EXACTLY before they can be adopted;
    // a colliding project's snapshot is never relabeled or overwritten.
    let qualified_legacy = dir.join(legacy_path_qualified_conversation_file_name(
        flow_id, flow_path,
    ));
    match crate::atomic_file::read_private_file(&qualified_legacy) {
        Ok(raw) => {
            let snapshot: PersistedFlowConversation = serde_json::from_str(&raw).ok()?;
            if snapshot.flow_id != flow_id || snapshot.flow_path != flow_path {
                return None;
            }
            let snapshot = canonicalize_persisted_conversation(
                snapshot,
                flow_id,
                flow_path,
                chrono::Utc::now(),
            )
            .ok()?
            .snapshot;
            // Claim the old name FIRST. Atomic same-directory rename closes the
            // race where two colliding projects both read an unconsumed legacy file.
            std::fs::rename(&qualified_legacy, &path).ok()?;
            let _ = persist_conversation_snapshot_to(dir, &snapshot);
            return Some(snapshot);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return None,
    }

    // Pre-path snapshots can carry an empty legacy path, but their embedded
    // flow ID must still match. Consume the shared name BEFORE returning any
    // private turns so another project can never adopt the same transcript.
    let legacy = dir.join(legacy_conversation_file_name(flow_id));
    let raw = crate::atomic_file::read_private_file(&legacy).ok()?;
    let snapshot: PersistedFlowConversation = serde_json::from_str(&raw).ok()?;
    if snapshot.flow_id != flow_id
        || (!snapshot.flow_path.is_empty() && snapshot.flow_path != flow_path)
    {
        return None;
    }
    if snapshot.version < SNAPSHOT_VERSION && snapshot.turns.is_empty() {
        let _ = std::fs::remove_file(&legacy);
        return None;
    }
    let snapshot =
        canonicalize_persisted_conversation(snapshot, flow_id, flow_path, chrono::Utc::now())
            .ok()?
            .snapshot;
    std::fs::rename(&legacy, &path).ok()?;
    let _ = persist_conversation_snapshot_to(dir, &snapshot);
    Some(snapshot)
}

/// One FIFO worker owns every conversation-store mutation (Oracle 2026-07-21
/// WP-A1): per-turn detached threads let an older snapshot finish AFTER a
/// newer one (silent transcript regression) and let a pending persist
/// resurrect a terminated conversation. Commands are enqueued synchronously
/// from the UI thread, so on-disk order always matches user-visible order.
pub struct FlowConversationStore {
    tx: std::sync::mpsc::Sender<ConversationStoreCommand>,
    #[cfg(test)]
    helper_revision: std::sync::atomic::AtomicU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationStoreReceipt {
    Written,
    IgnoredStaleRevision,
    IgnoredTombstonedThread,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationStoreError {
    ChannelClosed,
    Timeout,
    WriteFailed,
}

type ConversationStoreAck =
    std::sync::mpsc::Sender<Result<ConversationStoreReceipt, ConversationStoreError>>;

enum ConversationStoreCommand {
    Persist {
        snapshot: PersistedFlowConversation,
        ack: Option<ConversationStoreAck>,
    },
    PersistSelectedDeletion {
        snapshot: PersistedFlowConversation,
        deleted_thread_id: String,
        ack: Option<ConversationStoreAck>,
    },
    Flush(std::sync::mpsc::Sender<Result<(), ConversationStoreError>>),
}

#[derive(Default)]
struct ConversationStoreKeyState {
    highest_revision: u64,
    tombstoned_thread_ids: std::collections::HashSet<String>,
}

fn conversation_store_key(snapshot: &PersistedFlowConversation) -> (String, String) {
    (snapshot.flow_id.clone(), snapshot.flow_path.clone())
}

fn initial_conversation_store_state(
    dir: &std::path::Path,
    snapshot: &PersistedFlowConversation,
) -> ConversationStoreKeyState {
    let highest_revision =
        load_persisted_conversation_from(dir, &snapshot.flow_id, &snapshot.flow_path)
            .map_or(0, |persisted| persisted.revision);
    ConversationStoreKeyState {
        highest_revision,
        tombstoned_thread_ids: std::collections::HashSet::new(),
    }
}

/// Debug-only runtime seam for the stale-write negative control. When the app
/// is launched with SCRIPT_KIT_TEST_STATUS=1 and the named marker exists, the
/// FIFO worker publishes `<marker>.held` and pauses the next ordinary Persist
/// until the marker is removed. Selected deletion remains queued behind it,
/// proving that release cannot resurrect the tombstoned thread.
fn wait_for_flow_persist_test_release() {
    if std::env::var("SCRIPT_KIT_TEST_STATUS").ok().as_deref() != Some("1") {
        return;
    }
    let Some(marker) = std::env::var_os("SCRIPT_KIT_TEST_HOLD_FLOW_PERSIST_MARKER") else {
        return;
    };
    let marker = std::path::PathBuf::from(marker);
    if !marker.exists() {
        return;
    }
    let held = std::path::PathBuf::from(format!("{}.held", marker.to_string_lossy()));
    let _ = std::fs::write(&held, b"held");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while marker.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let _ = std::fs::remove_file(held);
}

fn persist_snapshot_in_worker(
    dir: &std::path::Path,
    states: &mut std::collections::HashMap<(String, String), ConversationStoreKeyState>,
    snapshot: PersistedFlowConversation,
    deleted_thread_id: Option<String>,
) -> Result<ConversationStoreReceipt, ConversationStoreError> {
    let key = conversation_store_key(&snapshot);
    let state = states
        .entry(key)
        .or_insert_with(|| initial_conversation_store_state(dir, &snapshot));
    if snapshot.revision <= state.highest_revision {
        return Ok(ConversationStoreReceipt::IgnoredStaleRevision);
    }
    if snapshot
        .threads
        .iter()
        .any(|thread| state.tombstoned_thread_ids.contains(&thread.id))
    {
        return Ok(ConversationStoreReceipt::IgnoredTombstonedThread);
    }
    if deleted_thread_id
        .as_ref()
        .is_some_and(|deleted| snapshot.threads.iter().any(|thread| &thread.id == deleted))
    {
        return Ok(ConversationStoreReceipt::IgnoredTombstonedThread);
    }
    if persist_conversation_snapshot_to(dir, &snapshot).is_err() {
        return Err(ConversationStoreError::WriteFailed);
    }
    state.highest_revision = snapshot.revision;
    if let Some(deleted_thread_id) = deleted_thread_id {
        state.tombstoned_thread_ids.insert(deleted_thread_id);
    }
    Ok(ConversationStoreReceipt::Written)
}

impl FlowConversationStore {
    /// Store rooted at `dir`. Tests construct their own with a temp dir; the
    /// app uses [`conversation_store`].
    pub fn new(dir: std::path::PathBuf) -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<ConversationStoreCommand>();
        let spawned = std::thread::Builder::new()
            .name("flow-conversation-store".into())
            .spawn(move || {
                let mut states = std::collections::HashMap::new();
                while let Ok(command) = rx.recv() {
                    match command {
                        ConversationStoreCommand::Persist { snapshot, ack } => {
                            wait_for_flow_persist_test_release();
                            let flow_id = snapshot.flow_id.clone();
                            let result =
                                persist_snapshot_in_worker(&dir, &mut states, snapshot, None);
                            if result.is_err() {
                                tracing::warn!(
                                    target: "script_kit::flows",
                                    event = "flow_conversation_persist_failed",
                                    flow_id = %flow_id,
                                    "Failed to persist flow conversation"
                                );
                            }
                            if let Some(ack) = ack {
                                let _ = ack.send(result);
                            }
                        }
                        ConversationStoreCommand::PersistSelectedDeletion {
                            snapshot,
                            deleted_thread_id,
                            ack,
                        } => {
                            let flow_id = snapshot.flow_id.clone();
                            let result = persist_snapshot_in_worker(
                                &dir,
                                &mut states,
                                snapshot,
                                Some(deleted_thread_id),
                            );
                            if result.is_err() {
                                tracing::warn!(
                                    target: "script_kit::flows",
                                    event = "flow_conversation_selected_delete_failed",
                                    flow_id = %flow_id,
                                    "Failed to persist selected Flow deletion"
                                );
                            }
                            if let Some(ack) = ack {
                                let _ = ack.send(result);
                            }
                        }
                        ConversationStoreCommand::Flush(done) => {
                            let _ = done.send(Ok(()));
                        }
                    }
                }
            });
        if let Err(err) = spawned {
            tracing::error!(
                target: "script_kit::flows",
                event = "flow_conversation_store_spawn_failed",
                error = %err,
                "Flow conversation store worker failed to start"
            );
        }
        Self {
            tx,
            #[cfg(test)]
            helper_revision: std::sync::atomic::AtomicU64::new(1),
        }
    }

    #[cfg(test)]
    pub fn persist(&self, flow_id: &str, flow_path: &str, turns: Vec<SessionTurn>) {
        let mut snapshot = snapshot_from_turns(flow_id, flow_path, &turns);
        snapshot.revision = self
            .helper_revision
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.persist_snapshot(snapshot);
    }

    pub fn persist_snapshot(&self, snapshot: PersistedFlowConversation) {
        let _ = self.tx.send(ConversationStoreCommand::Persist {
            snapshot,
            ack: None,
        });
    }

    pub fn persist_snapshot_and_wait(
        &self,
        snapshot: PersistedFlowConversation,
    ) -> Result<ConversationStoreReceipt, ConversationStoreError> {
        let (ack_tx, ack_rx) = std::sync::mpsc::channel();
        self.tx
            .send(ConversationStoreCommand::Persist {
                snapshot,
                ack: Some(ack_tx),
            })
            .map_err(|_| ConversationStoreError::ChannelClosed)?;
        ack_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .map_err(|_| ConversationStoreError::Timeout)?
    }

    pub fn persist_selected_deletion(
        &self,
        snapshot: PersistedFlowConversation,
        deleted_thread_id: String,
    ) {
        let _ = self
            .tx
            .send(ConversationStoreCommand::PersistSelectedDeletion {
                snapshot,
                deleted_thread_id,
                ack: None,
            });
    }

    pub fn persist_selected_deletion_and_wait(
        &self,
        snapshot: PersistedFlowConversation,
        deleted_thread_id: String,
    ) -> Result<ConversationStoreReceipt, ConversationStoreError> {
        let (ack_tx, ack_rx) = std::sync::mpsc::channel();
        self.tx
            .send(ConversationStoreCommand::PersistSelectedDeletion {
                snapshot,
                deleted_thread_id,
                ack: Some(ack_tx),
            })
            .map_err(|_| ConversationStoreError::ChannelClosed)?;
        ack_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .map_err(|_| ConversationStoreError::Timeout)?
    }

    /// Barrier: returns once every previously enqueued command has reached
    /// disk. Used by tests and shutdown.
    pub fn flush(&self) -> Result<(), ConversationStoreError> {
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        self.tx
            .send(ConversationStoreCommand::Flush(done_tx))
            .map_err(|_| ConversationStoreError::ChannelClosed)?;
        done_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .map_err(|_| ConversationStoreError::Timeout)?
    }
}

/// The app-wide store rooted at the active workspace.
pub fn conversation_store() -> &'static FlowConversationStore {
    static STORE: std::sync::OnceLock<FlowConversationStore> = std::sync::OnceLock::new();
    STORE.get_or_init(|| FlowConversationStore::new(conversation_store_dir()))
}

/// Persist under the active workspace (`~/.scriptkit`, `SK_PATH` override).
pub fn persist_conversation(flow_id: &str, flow_path: &str, turns: &[SessionTurn]) {
    if let Err(err) = persist_conversation_to(&conversation_store_dir(), flow_id, flow_path, turns)
    {
        tracing::warn!(
            target: "script_kit::flows",
            event = "flow_conversation_persist_failed",
            flow_id = %flow_id,
            error = %err,
            "Failed to persist flow conversation"
        );
    }
}

pub fn load_persisted_conversation(
    flow_id: &str,
    flow_path: &str,
) -> Option<PersistedFlowConversation> {
    load_persisted_conversation_from(&conversation_store_dir(), flow_id, flow_path)
}
