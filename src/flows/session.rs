//! Conversational flow sessions: metadata + turn-prompt building.
//!
//! A session is an agent conversation rendered with Script Kit's own chat
//! surface (`ChatPrompt`, the Threadline). No engine TUI is ever wrapped.
//! Two transports:
//!
//! - [`SessionTransport::CodexThread`] (flagship): codex-engine flows talk
//!   to a persistent `codex app-server` over JSON-RPC
//!   (`crate::flows::codex_client`). The first turn sends the flow's
//!   resolved mission (`resolve_flow_mission`); the protocol thread holds
//!   context, so later turns send the raw message.
//! - [`SessionTransport::MdflowTurns`] (second-class, non-codex engines):
//!   each user message launches one `md <flow> <prompt> --events` run (or
//!   `--_task <prompt>` only when an engine-free `md explain` proves that
//!   named contract). Streamed stdout fills the assistant bubble. mdflow runs
//!   are stateless, so context rides inside the turn prompt as a rolled-up
//!   transcript (`build_turn_task`).
//!
//! Contract (Conversation Desk):
//! - Enter on a flow = start (or resume) a conversation.
//! - Backgrounding NEVER kills a running turn; re-entering an Active row
//!   restores the SAME transcript entity.
//! - Stop cancels the in-flight turn only; the conversation survives.

/// Coarse session state, following Orca's attention model. Working while a
/// turn's events run is in flight; NeedsYou when the agent has replied and
/// the composer waits on the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// A turn is in flight (events run active).
    Working,
    /// The conversation is idle — the agent answered and awaits the user.
    NeedsYou,
    /// The last turn ended with this exit code (None = signal/unknown) and
    /// the user has not sent a new message since a failure worth surfacing.
    Done(Option<i32>),
}

impl SessionState {
    pub fn label(self) -> &'static str {
        match self {
            SessionState::Working => "working",
            SessionState::NeedsYou => "needs you",
            SessionState::Done(Some(0)) => "done",
            SessionState::Done(_) => "failed",
        }
    }

    pub fn is_live(self) -> bool {
        !matches!(self, SessionState::Done(_))
    }
}

/// One committed conversation turn, kept engine-agnostic for prompt rollup.
#[derive(Debug, Clone)]
pub struct SessionTurn {
    pub user: String,
    pub assistant: String,
}

/// How a session's turns reach an engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionTransport {
    /// Native `codex app-server` thread (codex-engine flows).
    CodexThread,
    /// One engine-free-resolved `md <flow> <turn-input> --events` registry run.
    MdflowTurns,
}

impl SessionTransport {
    pub fn for_engine(engine: &str) -> Self {
        if engine.eq_ignore_ascii_case("codex") {
            SessionTransport::CodexThread
        } else {
            SessionTransport::MdflowTurns
        }
    }
}

/// Requests posted from `ChatPrompt` callbacks (which have no app access)
/// and drained in the app render pass (window access for actions).
#[derive(Debug, Clone)]
pub enum FlowChatRequest {
    Submit { session_id: u64, text: String },
    Background { session_id: u64 },
    ShowActions { session_id: u64 },
}

/// Metadata for one conversation, independent of the GPUI entity.
#[derive(Debug, Clone)]
pub struct FlowSessionMeta {
    pub id: u64,
    pub flow_id: String,
    pub flow_name: String,
    pub friendly_name: String,
    pub origin: String,
    pub engine: String,
    /// Definition path (the flow's markdown file).
    pub flow_path: String,
    /// Definition mtime when the session's engine contract was resolved.
    /// Reattach compares against the file's current mtime: a drifted value
    /// marks the session `needs_rethread` so an edited flow never keeps a
    /// thread built from the old contract.
    pub flow_mtime_ms: u64,
    /// Cwd every turn runs in (pinned at session start).
    pub cwd: String,
    pub transport: SessionTransport,
    pub state: SessionState,
    pub started_at: std::time::Instant,
    /// Committed turns (user + final assistant text) for context rollup.
    pub turns: Vec<SessionTurn>,
    /// Active turn: transport bookkeeping + ChatPrompt streaming message id.
    pub active_turn: Option<ActiveTurn>,
    /// Codex thread transport: true once `thread/start` was answered — the
    /// footer shows "Connecting" instead of "Working" until then.
    pub thread_ready: bool,
    /// Codex thread transport: the engine died and the next submit lands on
    /// a FRESH protocol thread. The submit path must re-resolve the flow
    /// contract and carry the transcript rollup so the flow's identity and
    /// conversation survive — never continue as a generic new thread.
    pub needs_rethread: bool,
}

/// Bookkeeping for the in-flight turn.
#[derive(Debug, Clone)]
pub struct ActiveTurn {
    /// Registry run id for [`SessionTransport::MdflowTurns`]; `None` on the
    /// codex thread transport.
    pub run_id: Option<u64>,
    /// ChatPrompt streaming message this turn appends into.
    pub message_id: String,
    /// Assistant text forwarded so far (also the mdflow tail watermark).
    pub assistant_acc: String,
    /// Codex agentMessage item currently streaming (a turn can carry
    /// several items; boundaries render as paragraph breaks).
    pub current_item_id: Option<String>,
    /// Accumulated text of the CURRENT item only — `item/completed`
    /// reconciliation compares against this, never the whole turn.
    pub item_acc: String,
    pub user_text: String,
}

impl ActiveTurn {
    /// Enter an agentMessage item. A turn carries several items; when the
    /// stream moves to a NEW item this resets the per-item accumulator that
    /// `item/completed` reconciliation compares against, and returns true
    /// when a paragraph break must be appended first so consecutive items
    /// never butt-join ("…summarizing.The listed…"). Re-entering the
    /// current item is a no-op returning false.
    pub fn enter_item(&mut self, item_id: &str) -> bool {
        if self.current_item_id.as_deref() == Some(item_id) {
            return false;
        }
        self.current_item_id = Some(item_id.to_string());
        self.item_acc.clear();
        !self.assistant_acc.is_empty() && !self.assistant_acc.ends_with("\n\n")
    }
}

impl FlowSessionMeta {
    pub fn elapsed_label(&self) -> String {
        let secs = self.started_at.elapsed().as_secs();
        if secs < 60 {
            format!("{secs}s")
        } else if secs < 3600 {
            format!("{}m", secs / 60)
        } else {
            format!("{}h", secs / 3600)
        }
    }
}

/// Cap on rolled-up history characters per turn prompt. Oldest turns fall
/// off first; the newest message always survives intact.
const HISTORY_CHAR_BUDGET: usize = 8_000;

/// Build the mdflow turn prompt: prior transcript (newest-biased, budgeted)
/// then the new message. First turn passes the message verbatim so simple
/// one-shot flows behave exactly like the CLI.
pub fn build_turn_task(turns: &[SessionTurn], message: &str) -> String {
    if turns.is_empty() {
        return message.to_string();
    }
    let mut history: Vec<String> = Vec::new();
    let mut used = 0usize;
    for turn in turns.iter().rev() {
        let block = format!("User: {}\nAssistant: {}", turn.user, turn.assistant);
        if used + block.len() > HISTORY_CHAR_BUDGET {
            break;
        }
        used += block.len();
        history.push(block);
    }
    history.reverse();
    format!(
        "Conversation so far (for context):\n\n{}\n\nReply to the user's new message:\n{}",
        history.join("\n\n"),
        message
    )
}

/// The one user-prompt argument for a non-Codex Threadline turn.
///
/// Positional `_1` is mdflow's ordinary/public flow contract. `NamedTask` is
/// selected only after positional explain fails and a second, engine-free
/// explain proves that the flow consumes `{{ _task }}`. This prevents an
/// unused `--_task` flag from starving required `_1` before engine launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MdflowTurnArg {
    Positional(String),
    NamedTask(String),
}

/// Resolve the turn argument shape without launching the flow's engine.
/// Prefer the public positional contract; fall back to `--_task` only when
/// `md explain` accepts that named input. If neither shape resolves, fail
/// closed rather than risk launching an engine with a guessed prompt shape.
pub fn resolve_mdflow_turn_arg(
    binary: &str,
    flow_path: &str,
    cwd: &str,
    prompt: &str,
) -> Result<MdflowTurnArg, String> {
    let explain = |named_task: bool| {
        let mut command = std::process::Command::new(binary);
        command.arg("explain").arg(flow_path);
        if named_task {
            command.arg("--_task").arg(prompt);
        } else {
            command.arg(prompt);
        }
        command
            .arg("--json")
            .current_dir(cwd)
            .stdin(std::process::Stdio::null())
            .output()
    };

    let positional = explain(false)
        .map_err(|err| format!("mdflow explain failed to spawn for positional input: {err}"))?;
    if positional.status.success() {
        return Ok(MdflowTurnArg::Positional(prompt.to_string()));
    }

    let named = explain(true)
        .map_err(|err| format!("mdflow explain failed to spawn for --_task input: {err}"))?;
    if named.status.success() {
        return Ok(MdflowTurnArg::NamedTask(prompt.to_string()));
    }

    let diagnostic = |output: &std::process::Output| {
        String::from_utf8_lossy(&output.stderr)
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("mdflow explain rejected the input")
            .to_string()
    };
    Err(format!(
        "mdflow explain rejected both Threadline input shapes (positional: {}; --_task: {})",
        diagnostic(&positional),
        diagnostic(&named)
    ))
}

/// Resolve a flow's mission for the FIRST codex-thread turn: frontmatter
/// stripped, `{{ _task }}` substituted with the user's message (appended
/// when the template has no task slot). Later turns send the raw message —
/// the protocol thread holds context.
///
/// This is a deliberate v1 of mdflow's own resolution (`md explain --json`
/// is the robust path once its output is cached per flow); flows in
/// `@johnlindquist/flows` are frontmatter + prose + `{{ _task }}`.
/// Frontmatter contract a flow pins for its conversation thread. Passed
/// into `thread/start` so a codex session honors the flow's declared
/// `model:`/`sandbox:` instead of silently falling back to the user's
/// global codex defaults, and carries the mission as developer
/// instructions so it survives every turn (and engine re-threads).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlowThreadProfile {
    pub model: Option<String>,
    pub sandbox: Option<String>,
    pub developer_instructions: Option<String>,
}

/// Resolved first-turn contract for the codex thread transport.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowThreadContract {
    pub profile: FlowThreadProfile,
    pub first_prompt: String,
}

/// Resolve a flow definition into its thread contract. Bodies that template
/// `{{ _task }}` keep the legacy shape (mission+task as the first prompt);
/// plain bodies become persistent developer instructions and the first
/// prompt stays the user's own words.
pub fn resolve_flow_thread_contract(markdown: &str, task: &str) -> FlowThreadContract {
    let (model, sandbox) = parse_frontmatter_overrides(markdown);
    let body = strip_frontmatter(markdown).trim();
    let templated = body.contains("{{ _task }}") || body.contains("{{_task}}");
    if templated || body.is_empty() {
        FlowThreadContract {
            profile: FlowThreadProfile {
                model,
                sandbox,
                developer_instructions: None,
            },
            first_prompt: resolve_flow_mission(markdown, task),
        }
    } else {
        FlowThreadContract {
            profile: FlowThreadProfile {
                model,
                sandbox,
                developer_instructions: Some(body.to_string()),
            },
            first_prompt: task.to_string(),
        }
    }
}

/// Minimal frontmatter scan for the two keys codex `thread/start` accepts.
/// Values pass through verbatim (quotes stripped) — an invalid sandbox mode
/// should fail the thread start loudly, never be silently dropped.
fn parse_frontmatter_overrides(markdown: &str) -> (Option<String>, Option<String>) {
    let Some(rest) = markdown.strip_prefix("---") else {
        return (None, None);
    };
    let Some(end) = rest.find("\n---") else {
        return (None, None);
    };
    let block = &rest[..end];
    let mut model = None;
    let mut sandbox = None;
    for line in block.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim().trim_matches('"').trim_matches('\'');
        if value.is_empty() {
            continue;
        }
        match key.trim() {
            "model" => model = Some(value.to_string()),
            "sandbox" => sandbox = Some(value.to_string()),
            _ => {}
        }
    }
    (model, sandbox)
}

pub fn resolve_flow_mission(markdown: &str, task: &str) -> String {
    let body = strip_frontmatter(markdown).trim();
    let with_task = if body.contains("{{ _task }}") || body.contains("{{_task}}") {
        body.replace("{{ _task }}", task).replace("{{_task}}", task)
    } else if body.is_empty() {
        task.to_string()
    } else {
        format!("{body}\n\n{task}")
    };
    with_task.trim().to_string()
}

fn strip_frontmatter(markdown: &str) -> &str {
    let Some(rest) = markdown.strip_prefix("---") else {
        return markdown;
    };
    match rest.find("\n---") {
        Some(end) => {
            let after = &rest[end + 4..];
            after.strip_prefix('\n').unwrap_or(after)
        }
        None => markdown,
    }
}

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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PersistedFlowConversation {
    pub flow_id: String,
    /// Definition path this conversation belongs to (empty on legacy
    /// snapshots persisted before identity was path-qualified).
    #[serde(default)]
    pub flow_path: String,
    pub saved_at: String,
    pub turns: Vec<PersistedFlowTurn>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PersistedFlowTurn {
    pub user: String,
    pub assistant: String,
}

/// Newest persisted turns kept per flow — comfortably above what
/// `build_turn_task`'s `HISTORY_CHAR_BUDGET` can roll up, without letting
/// the snapshot grow unbounded.
const PERSISTED_TURN_CAP: usize = 12;

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

/// Conversation file name: flow id PLUS definition path, so two projects
/// with the same `project:review` id can never share (or steal) a
/// transcript. The path portion keeps its most distinctive tail when it
/// would push the file name over conservative filesystem limits.
fn conversation_file_name(flow_id: &str, flow_path: &str) -> String {
    let id = sanitize_component(flow_id);
    let mut path = sanitize_component(flow_path.trim_start_matches('/'));
    const PATH_PORTION_MAX: usize = 160;
    if path.len() > PATH_PORTION_MAX {
        path = path[path.len() - PATH_PORTION_MAX..].to_string();
    }
    format!("{id}--{path}.json")
}

/// Legacy (pre path-qualified identity) file name, keyed by flow id alone.
fn legacy_conversation_file_name(flow_id: &str) -> String {
    format!("{}.json", sanitize_component(flow_id))
}

pub fn persist_conversation_to(
    dir: &std::path::Path,
    flow_id: &str,
    flow_path: &str,
    turns: &[SessionTurn],
) -> std::io::Result<()> {
    let kept: Vec<PersistedFlowTurn> = turns
        .iter()
        .rev()
        .take(PERSISTED_TURN_CAP)
        .rev()
        .map(|turn| PersistedFlowTurn {
            user: turn.user.clone(),
            assistant: turn.assistant.clone(),
        })
        .collect();
    let snapshot = PersistedFlowConversation {
        flow_id: flow_id.to_string(),
        flow_path: flow_path.to_string(),
        saved_at: chrono::Utc::now().to_rfc3339(),
        turns: kept,
    };
    std::fs::create_dir_all(dir)?;
    let path = dir.join(conversation_file_name(flow_id, flow_path));
    // Unique-temp atomic write (see src/atomic_file.rs): a fixed temp path let
    // concurrent savers corrupt the persisted conversation.
    let bytes = serde_json::to_vec_pretty(&snapshot).map_err(std::io::Error::other)?;
    crate::atomic_file::write_atomic(&path, &bytes)
}

pub fn load_persisted_conversation_from(
    dir: &std::path::Path,
    flow_id: &str,
    flow_path: &str,
) -> Option<PersistedFlowConversation> {
    let path = dir.join(conversation_file_name(flow_id, flow_path));
    if let Ok(raw) = std::fs::read_to_string(&path) {
        let snapshot: PersistedFlowConversation = serde_json::from_str(&raw).ok()?;
        return (!snapshot.turns.is_empty()).then_some(snapshot);
    }
    // Legacy adoption (one-shot): a pre-identity snapshot keyed by id alone
    // is claimed by the FIRST flow that opens it, re-persisted under the
    // path-qualified name, and the legacy file removed — so it can never
    // silently leak into another project again.
    let legacy = dir.join(legacy_conversation_file_name(flow_id));
    let raw = std::fs::read_to_string(&legacy).ok()?;
    let snapshot: PersistedFlowConversation = serde_json::from_str(&raw).ok()?;
    if snapshot.turns.is_empty() {
        return None;
    }
    let turns: Vec<SessionTurn> = snapshot
        .turns
        .iter()
        .map(|turn| SessionTurn {
            user: turn.user.clone(),
            assistant: turn.assistant.clone(),
        })
        .collect();
    if persist_conversation_to(dir, flow_id, flow_path, &turns).is_ok() {
        let _ = std::fs::remove_file(&legacy);
    }
    Some(PersistedFlowConversation {
        flow_path: flow_path.to_string(),
        ..snapshot
    })
}

/// Erase a conversation's persisted history (both the path-qualified file
/// and any legacy id-only file). "Terminate Flow" promises permanent
/// removal — before this existed, the next activation silently restored the
/// supposedly terminated conversation (2026-07-11 audit P0).
pub fn delete_persisted_conversation_from(dir: &std::path::Path, flow_id: &str, flow_path: &str) {
    let _ = std::fs::remove_file(dir.join(conversation_file_name(flow_id, flow_path)));
    let _ = std::fs::remove_file(dir.join(legacy_conversation_file_name(flow_id)));
}

/// Persist under the active workspace (`~/.scriptkit`, `SK_PATH` override).
pub fn persist_conversation(flow_id: &str, flow_path: &str, turns: &[SessionTurn]) {
    if turns.is_empty() {
        return;
    }
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

pub fn delete_persisted_conversation(flow_id: &str, flow_path: &str) {
    delete_persisted_conversation_from(&conversation_store_dir(), flow_id, flow_path);
}

/// Definition-file mtime in ms (0 when unreadable) — the cheap staleness
/// signal for reattaching live sessions: an edited flow must not silently
/// keep a thread built from the old contract.
pub fn flow_definition_mtime_ms(path: &str) -> u64 {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mission_resolution_strips_frontmatter_and_substitutes_task() {
        let markdown =
            "---\ndescription: GitHub examples\n---\nSearch GitHub for examples.\n\n{{ _task }}\n";
        assert_eq!(
            resolve_flow_mission(markdown, "bun shell scripts"),
            "Search GitHub for examples.\n\nbun shell scripts"
        );
    }

    #[test]
    fn mission_without_task_slot_appends_message() {
        assert_eq!(
            resolve_flow_mission("Reply tersely.", "hello"),
            "Reply tersely.\n\nhello"
        );
        assert_eq!(resolve_flow_mission("", "hello"), "hello");
    }

    #[test]
    fn transport_picks_codex_thread_only_for_codex() {
        assert_eq!(
            SessionTransport::for_engine("codex"),
            SessionTransport::CodexThread
        );
        assert_eq!(
            SessionTransport::for_engine("claude"),
            SessionTransport::MdflowTurns
        );
        assert_eq!(
            SessionTransport::for_engine("fasteng"),
            SessionTransport::MdflowTurns
        );
    }

    #[test]
    fn first_turn_task_is_verbatim() {
        assert_eq!(
            build_turn_task(&[], "what did vercel email me?"),
            "what did vercel email me?"
        );
    }

    const GMAIL_PATH: &str = "/pkg/flows/flow-gog-gmail.codex.md";

    #[test]
    fn conversation_persistence_round_trips_and_caps_turns() {
        let dir = tempfile::tempdir().expect("tempdir");
        let turns: Vec<SessionTurn> = (0..20)
            .map(|i| SessionTurn {
                user: format!("question {i}"),
                assistant: format!("answer {i}"),
            })
            .collect();
        persist_conversation_to(dir.path(), "package:flow-gog-gmail", GMAIL_PATH, &turns)
            .expect("persist");

        let restored =
            load_persisted_conversation_from(dir.path(), "package:flow-gog-gmail", GMAIL_PATH)
                .expect("snapshot must load");
        assert_eq!(restored.flow_id, "package:flow-gog-gmail");
        assert_eq!(restored.flow_path, GMAIL_PATH);
        // Newest PERSISTED_TURN_CAP turns survive, oldest fall off.
        assert_eq!(restored.turns.len(), 12);
        assert_eq!(restored.turns.first().unwrap().user, "question 8");
        assert_eq!(restored.turns.last().unwrap().assistant, "answer 19");
    }

    #[test]
    fn persisted_conversation_missing_or_empty_is_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(
            load_persisted_conversation_from(dir.path(), "project:scout", "/a/flows/scout.md")
                .is_none()
        );
        persist_conversation_to(dir.path(), "project:scout", "/a/flows/scout.md", &[])
            .expect("persist empty");
        assert!(
            load_persisted_conversation_from(dir.path(), "project:scout", "/a/flows/scout.md")
                .is_none(),
            "an empty snapshot must never restore a blank conversation"
        );
    }

    /// Two projects with the same `project:review` id must never share a
    /// transcript (2026-07-11 audit P0: cross-project restore was both a
    /// correctness and privacy failure).
    #[test]
    fn same_flow_id_in_different_projects_gets_separate_transcripts() {
        let dir = tempfile::tempdir().expect("tempdir");
        let turn = |text: &str| {
            vec![SessionTurn {
                user: text.to_string(),
                assistant: format!("re: {text}"),
            }]
        };
        persist_conversation_to(
            dir.path(),
            "project:review",
            "/work/alpha/flows/review.md",
            &turn("alpha secrets"),
        )
        .expect("persist alpha");
        persist_conversation_to(
            dir.path(),
            "project:review",
            "/work/beta/flows/review.md",
            &turn("beta question"),
        )
        .expect("persist beta");

        let alpha = load_persisted_conversation_from(
            dir.path(),
            "project:review",
            "/work/alpha/flows/review.md",
        )
        .expect("alpha loads");
        let beta = load_persisted_conversation_from(
            dir.path(),
            "project:review",
            "/work/beta/flows/review.md",
        )
        .expect("beta loads");
        assert_eq!(alpha.turns[0].user, "alpha secrets");
        assert_eq!(beta.turns[0].user, "beta question");
    }

    /// Legacy id-only snapshots are adopted once (re-keyed under the
    /// path-qualified name, legacy file removed) so they can never leak
    /// into a second project later.
    #[test]
    fn legacy_snapshot_is_adopted_once_and_removed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let legacy = dir
            .path()
            .join(legacy_conversation_file_name("project:review"));
        let snapshot = PersistedFlowConversation {
            flow_id: "project:review".into(),
            flow_path: String::new(),
            saved_at: "2026-07-10T00:00:00Z".into(),
            turns: vec![PersistedFlowTurn {
                user: "old question".into(),
                assistant: "old answer".into(),
            }],
        };
        std::fs::write(&legacy, serde_json::to_vec_pretty(&snapshot).unwrap()).unwrap();

        let adopted = load_persisted_conversation_from(
            dir.path(),
            "project:review",
            "/work/alpha/flows/review.md",
        )
        .expect("legacy snapshot adopted");
        assert_eq!(adopted.turns[0].user, "old question");
        assert_eq!(adopted.flow_path, "/work/alpha/flows/review.md");
        assert!(!legacy.exists(), "legacy file must be consumed");
        assert!(
            load_persisted_conversation_from(
                dir.path(),
                "project:review",
                "/work/beta/flows/review.md",
            )
            .is_none(),
            "a second project must not inherit the adopted transcript"
        );
    }

    /// Terminate promises "permanently end this conversation" — deletion
    /// must actually remove the persisted history (2026-07-11 audit P0).
    #[test]
    fn delete_erases_persisted_history_including_legacy() {
        let dir = tempfile::tempdir().expect("tempdir");
        let turns = vec![SessionTurn {
            user: "q".into(),
            assistant: "a".into(),
        }];
        persist_conversation_to(dir.path(), "project:review", "/w/flows/review.md", &turns)
            .expect("persist");
        let legacy = dir
            .path()
            .join(legacy_conversation_file_name("project:review"));
        std::fs::write(&legacy, b"{}").unwrap();

        delete_persisted_conversation_from(dir.path(), "project:review", "/w/flows/review.md");
        assert!(load_persisted_conversation_from(
            dir.path(),
            "project:review",
            "/w/flows/review.md"
        )
        .is_none());
        assert!(!legacy.exists());
    }

    #[test]
    fn later_turns_roll_up_history_then_message() {
        let turns = vec![SessionTurn {
            user: "find bun shell examples".into(),
            assistant: "Here are three repos …".into(),
        }];
        let task = build_turn_task(&turns, "show me the second one");
        assert!(task.starts_with("Conversation so far"));
        assert!(task.contains("User: find bun shell examples"));
        assert!(task.contains("Assistant: Here are three repos …"));
        assert!(task.ends_with("show me the second one"));
    }

    #[test]
    fn history_budget_drops_oldest_turns_first() {
        let big = "x".repeat(6_000);
        let turns = vec![
            SessionTurn {
                user: "oldest".into(),
                assistant: big.clone(),
            },
            SessionTurn {
                user: "newest".into(),
                assistant: big,
            },
        ];
        let task = build_turn_task(&turns, "next");
        assert!(!task.contains("oldest"));
        assert!(task.contains("newest"));
        assert!(task.ends_with("next"));
    }

    #[test]
    fn templated_flow_contract_keeps_mission_in_first_prompt() {
        let markdown =
            "---\ndescription: GitHub examples\n---\nSearch GitHub for examples.\n\n{{ _task }}\n";
        let contract = resolve_flow_thread_contract(markdown, "bun shell scripts");
        assert_eq!(contract.profile.developer_instructions, None);
        assert_eq!(
            contract.first_prompt,
            "Search GitHub for examples.\n\nbun shell scripts"
        );
    }

    #[test]
    fn plain_flow_contract_pins_mission_as_developer_instructions() {
        let markdown = "---\ndescription: Terse\n---\nYou are gmail-agent. Reply tersely.\n";
        let contract = resolve_flow_thread_contract(markdown, "what did vercel email me?");
        assert_eq!(
            contract.profile.developer_instructions.as_deref(),
            Some("You are gmail-agent. Reply tersely.")
        );
        assert_eq!(contract.first_prompt, "what did vercel email me?");
    }

    #[test]
    fn empty_body_contract_sends_task_verbatim_with_no_instructions() {
        let contract = resolve_flow_thread_contract("---\nmodel: gpt-5\n---\n", "hello");
        assert_eq!(contract.profile.developer_instructions, None);
        assert_eq!(contract.first_prompt, "hello");
    }

    #[test]
    fn frontmatter_model_and_sandbox_pass_through_with_quotes_stripped() {
        let markdown =
            "---\nmodel: \"gpt-5.6-sol\"\nsandbox: 'read-only'\nother: x\n---\nMission.\n";
        let contract = resolve_flow_thread_contract(markdown, "go");
        assert_eq!(contract.profile.model.as_deref(), Some("gpt-5.6-sol"));
        assert_eq!(contract.profile.sandbox.as_deref(), Some("read-only"));
        let bare = resolve_flow_thread_contract("Mission.", "go");
        assert_eq!(bare.profile.model, None);
        assert_eq!(bare.profile.sandbox, None);
    }

    #[test]
    fn rethread_contract_carries_mission_and_transcript_rollup() {
        // Engine died mid-conversation: the submit path resolves the
        // contract again with build_turn_task(turns, message) as the task,
        // so the fresh thread gets BOTH the flow's identity and the prior
        // conversation — never a generic new thread.
        let turns = vec![SessionTurn {
            user: "find bun shell examples".into(),
            assistant: "Here are three repos …".into(),
        }];
        let rollup = build_turn_task(&turns, "show me the second one");

        let plain = resolve_flow_thread_contract("You are repo-scout.", &rollup);
        assert_eq!(
            plain.profile.developer_instructions.as_deref(),
            Some("You are repo-scout.")
        );
        assert!(plain.first_prompt.contains("User: find bun shell examples"));
        assert!(plain.first_prompt.ends_with("show me the second one"));

        let templated = resolve_flow_thread_contract("Scout repos.\n\n{{ _task }}", &rollup);
        assert!(templated.first_prompt.starts_with("Scout repos."));
        assert!(templated
            .first_prompt
            .contains("Assistant: Here are three repos …"));
    }

    fn active_turn(assistant_acc: &str) -> ActiveTurn {
        ActiveTurn {
            run_id: None,
            message_id: "m".into(),
            assistant_acc: assistant_acc.into(),
            current_item_id: None,
            item_acc: String::new(),
            user_text: "u".into(),
        }
    }

    #[test]
    fn entering_a_new_item_after_text_needs_a_paragraph_break() {
        let mut turn = active_turn("First item ends with a period.");
        turn.current_item_id = Some("item-1".into());
        turn.item_acc = "First item ends with a period.".into();
        assert!(turn.enter_item("item-2"));
        assert_eq!(turn.item_acc, "");
        assert_eq!(turn.current_item_id.as_deref(), Some("item-2"));
    }

    #[test]
    fn same_item_and_first_item_never_break() {
        let mut turn = active_turn("");
        assert!(
            !turn.enter_item("item-1"),
            "first item: nothing to separate"
        );
        turn.item_acc = "streaming".into();
        assert!(!turn.enter_item("item-1"), "same item: no-op");
        assert_eq!(
            turn.item_acc, "streaming",
            "same item must keep its accumulator"
        );
    }

    #[test]
    fn existing_paragraph_break_is_not_doubled() {
        let mut turn = active_turn("First item.\n\n");
        turn.current_item_id = Some("item-1".into());
        assert!(!turn.enter_item("item-2"));
    }

    #[test]
    fn state_labels_are_honest() {
        assert_eq!(SessionState::Working.label(), "working");
        assert_eq!(SessionState::NeedsYou.label(), "needs you");
        assert_eq!(SessionState::Done(Some(0)).label(), "done");
        assert_eq!(SessionState::Done(Some(1)).label(), "failed");
        assert!(SessionState::Working.is_live());
        assert!(SessionState::NeedsYou.is_live());
        assert!(!SessionState::Done(None).is_live());
    }
}
