// Flow Desk (Conversation Desk → Threadline, 2026-07-09).
//
// Every flow is an agent identity. One main-window surface over the shared
// flow substrate (`crate::flows`):
//
// - Enter on a flow = CONVERSE: open a Threadline session — Script Kit's
//   own `ChatPrompt` transcript + composer. No engine TUI is ever wrapped.
//   Codex-engine flows talk to a persistent `codex app-server` thread
//   (`crate::flows::codex_client`); other engines run one
//   `md <flow> --_task … --events` registry run per turn (second-class).
// - Enter on an Active session row = reattach the SAME transcript entity.
// - ⇧↵ = run once in the background via `md <flow> --events` (registry).
// - Esc in a session = background (never kills); ⌘⇧D does the same. Esc in
//   the desk clears the filter / goes back. Stop is an explicit ⌘K verb
//   that cancels only the in-flight turn.
//
// The detached Flow Manager and the Flash/Dispatch/Lens/Mission-Control
// variants are dead; `FlowUxVariant` survives only as builtin-entry plumbing.

/// One selectable row in the desk list.
#[derive(Clone)]
pub(crate) enum FlowDeskRow {
    /// Live or recently-ended conversation (index into `flow_sessions`).
    Session(u64),
    /// A background registry run (run-once / workflow) by local id — runs
    /// are supervised IN the desk: phase, elapsed, last output, ⌘K cancel.
    Run(u64),
    /// A flow identity from the combined roster+package corpus.
    Flow(Box<crate::flows::model::FlowDescriptor>),
    /// mdflow missing: the actionable install affordance (Enter runs the
    /// install in the Quick Terminal).
    InstallMdflow,
    /// mdflow present but pre-protocol: offer the explicit upgrade command.
    UpgradeMdflow,
    /// A typed roster failure: retry discovery without exposing raw stderr.
    RetryRoster,
    /// A nonempty query matched no session, run, or flow: clear it.
    ClearQuery,
    /// mdflow present but the roster is empty: offer the `md init` starter
    /// scaffold.
    InitFlows,
    /// The plain-English creation affordance.
    CreateFlow,
}

/// Typed desk/setup state. Rendering, automation, recovery rows, and tests all
/// consume this value rather than reverse-engineering status from display copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FlowDeskState {
    Loading,
    MdflowMissing,
    MdflowIncompatible,
    RosterFailed {
        failure: crate::ai::reliability::AppFailureRecord,
    },
    ReadyEmpty,
    NoMatch,
    Ready,
}

impl FlowDeskState {
    pub(crate) fn automation_label(&self) -> &'static str {
        match self {
            Self::Loading => "Loading",
            Self::MdflowMissing => "MdflowMissing",
            Self::MdflowIncompatible => "MdflowIncompatible",
            Self::RosterFailed { .. } => "RosterFailed",
            Self::ReadyEmpty => "ReadyEmpty",
            Self::NoMatch => "NoMatch",
            Self::Ready => "Ready",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlowDeskRowVerb {
    OpenConversation,
    OpenRunActions,
    Converse,
    OpenInTerminal,
    RunOnce,
    InstallMdflow,
    UpgradeMdflow,
    RetryRoster,
    ScaffoldFlows,
    ClearSearch,
    CreateFlow,
}

impl FlowDeskRowVerb {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::OpenConversation => "Open Conversation",
            Self::OpenRunActions => "Open Run Actions",
            Self::Converse => "Converse",
            Self::OpenInTerminal => "Open in Terminal",
            Self::RunOnce => "Run Once",
            Self::InstallMdflow => "Install mdflow",
            Self::UpgradeMdflow => "Upgrade mdflow",
            Self::RetryRoster => "Retry",
            Self::ScaffoldFlows => "Scaffold",
            Self::ClearSearch => "Clear Search",
            Self::CreateFlow => "Create Flow",
        }
    }
}

/// Single selected-row projection consumed by paint, semantics, footer, and
/// activation. A row cannot advertise a verb that its Enter path does not own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FlowDeskRowDescriptor {
    pub semantic_id: String,
    pub title: String,
    pub detail: String,
    pub icon: &'static str,
    pub primary: FlowDeskRowVerb,
    pub secondary: Option<FlowDeskRowVerb>,
    pub actions_available: bool,
}

/// How a settled turn presents in the transcript: normal completion, a quiet
/// user-initiated stop (never the red error treatment), or a real typed
/// failure (S09 — the classified record survives to persistence/recovery).
#[derive(Clone)]
pub(crate) enum FlowTurnOutcome {
    Ok,
    Stopped,
    Failed(crate::ai::reliability::AppFailureRecord),
}

include!("flow_ux_session_presentation.rs");

/// Capabilities constructed only by the Actions confirmation closures. The
/// mutating lifecycle functions require these tokens, so dismissal and footer
/// paths cannot accidentally become destructive.
pub(crate) struct ConfirmedFlowThreadDeletion(());
pub(crate) struct ConfirmedFlowRuntimeTermination(());

/// Exactly one action a flow-session key press resolves to (C-R1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FlowSessionKeyAction {
    /// ⎋ — leave the session view without touching the process.
    Background,
    /// ⌘. — cancel the in-flight turn only (conversation survives).
    Stop,
    /// ⌘K — open/close the session actions menu.
    ToggleActions,
    /// ⌘L — start a new conversation with this flow, same chord Agent Chat
    /// uses. Resolved even while a turn is in flight: the handler owns the
    /// refusal so the "is this allowed" rule lives in ONE place.
    NewConversation,
    /// ⇧⌘C — copy the newest assistant answer, the same chord Agent Chat uses
    /// and the same chord this session's ⌘K menu already advertises.
    CopyLastResponse,
    /// Plain, unmodified ↵ — send the composer draft as the next turn.
    Submit,
    /// No shell-level action; the key falls through to the composer input.
    Ignore,
}

/// The single exhaustive key owner for a flow session (C-R1).
///
/// WP7 deleted `ChatPrompt`'s own key handling for transcript-only hosts, so
/// the flow-session parent handler is now the ONE lifecycle/key owner. It must
/// therefore resolve every binding here — including ⌘. Stop, which WP7 dropped,
/// and the plain-Enter guard that keeps Shift+Enter / Cmd+Enter from
/// over-submitting the draft. See `resolve_chat_input_key_action` for the
/// standalone-host parity reference.
///
/// Precedence: while the actions popup is open, Escape belongs to the popup, so
/// Background requires `!actions_open`. ⌘. Stop only fires while a
/// turn is in flight; ⌘K always toggles; only a bare Enter submits.
/// Held against the ⌘K action list by
/// `flow_desk_create_discoverability_tests::every_advertised_session_shortcut_has_a_declared_owner`.
/// A shortcut badge is a promise, and the only thing keeping the badge and the
/// binding from drifting apart is that one test reads both.
fn resolve_flow_session_key_action(
    key: &str,
    platform: bool,
    shift: bool,
    facts: crate::components::conversation_actions::FlowConversationCommandFacts,
    actions_open: bool,
) -> FlowSessionKeyAction {
    use crate::components::conversation_actions::{
        flow_conversation_commands_for_facts, match_conversation_command_shortcut,
        ConversationCommandAvailability, FlowConversationCommand,
    };

    if platform && key.eq_ignore_ascii_case("k") {
        return FlowSessionKeyAction::ToggleActions;
    }
    if actions_open {
        return FlowSessionKeyAction::Ignore;
    }

    match match_conversation_command_shortcut(
        &flow_conversation_commands_for_facts(facts),
        key,
        platform,
        shift,
    ) {
        // Consume ⌘L even while disabled so a refused New cannot type "l" into
        // the composer. The transition handler rechecks active work.
        Some((FlowConversationCommand::NewConversation, _)) => {
            FlowSessionKeyAction::NewConversation
        }
        Some((FlowConversationCommand::Background, ConversationCommandAvailability::Enabled)) => {
            FlowSessionKeyAction::Background
        }
        Some((FlowConversationCommand::Stop, ConversationCommandAvailability::Enabled)) => {
            FlowSessionKeyAction::Stop
        }
        Some((
            FlowConversationCommand::CopyLastResponse,
            ConversationCommandAvailability::Enabled,
        )) => FlowSessionKeyAction::CopyLastResponse,
        Some((FlowConversationCommand::Send, ConversationCommandAvailability::Enabled)) => {
            FlowSessionKeyAction::Submit
        }
        Some((_, ConversationCommandAvailability::Disabled { .. }))
        | Some((
            FlowConversationCommand::BackToCurrent
            | FlowConversationCommand::ConversationHistory
            | FlowConversationCommand::ContinueAsNewConversation
            | FlowConversationCommand::DeleteConversation
            | FlowConversationCommand::TerminateRuntime,
            ConversationCommandAvailability::Enabled,
        ))
        | None => FlowSessionKeyAction::Ignore,
    }
}

/// What Up/Down should do to a flow session's composer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FlowPromptHistoryMove {
    /// Leave the composer alone and let the key fall through — there is no
    /// history, or the user is already on their live draft and pressed Down.
    Ignore,
    /// Put history entry `index` in the composer.
    Recall(usize),
    /// Arrowed back past the newest entry: restore the draft the user was
    /// typing before recall started.
    RestoreDraft,
}

/// Where Up/Down moves through a flow session's prompt history.
///
/// Agent Chat has recalled previous prompts for a long time; Flow never did,
/// because the arrow interceptor has an arm for `FlowUxView` but none for
/// `FlowSessionView`, so arrows fell through to the catch-all. Retyping a long
/// prompt to tweak one word is the everyday cost of that gap.
///
/// `history` is ordered oldest → newest, matching a session's turns.
///
/// The rules follow shell history, which is what a user's fingers already
/// expect:
///
/// - Up from the live draft recalls the NEWEST entry, not the oldest.
/// - Up at the oldest entry stays there rather than wrapping — wrapping makes
///   a long history feel like it lost your place.
/// - Down past the newest entry restores the draft, so recall is always
///   reversible without retyping.
/// - Down while already on the draft does nothing.
pub(crate) fn flow_prompt_history_move(
    history_len: usize,
    current: Option<usize>,
    is_up: bool,
) -> FlowPromptHistoryMove {
    if history_len == 0 {
        return FlowPromptHistoryMove::Ignore;
    }

    match (current, is_up) {
        // Entering history from the draft: newest first.
        (None, true) => FlowPromptHistoryMove::Recall(history_len - 1),
        (None, false) => FlowPromptHistoryMove::Ignore,
        // Older, clamped at the oldest entry.
        (Some(index), true) => FlowPromptHistoryMove::Recall(index.saturating_sub(1)),
        // Newer, then back out to the draft.
        (Some(index), false) => {
            if index + 1 < history_len {
                FlowPromptHistoryMove::Recall(index + 1)
            } else {
                FlowPromptHistoryMove::RestoreDraft
            }
        }
    }
}

/// Outcome of `submit_flow_chat_message`, so the caller can decide whether to
/// clear the composer. Callers must only clear the draft when
/// `consumes_draft()` — clearing before submit destroys the user's message
/// when the session is busy (WP1, 2026-07-21 Oracle panel P0).
#[must_use = "the result determines whether the composer draft was consumed"]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FlowChatSubmitResult {
    /// The turn was accepted and dispatched to the engine.
    Dispatched,
    /// The flow definition was unreadable: the message was consumed into one
    /// failed-closed turn (user row + structured error) without dispatch.
    FailedClosed,
    /// The submitted text was empty after trimming; nothing happened.
    Empty,
    /// A turn is already in flight on this session; the draft is preserved.
    Busy,
    /// Archived transcripts are immutable; Continue as New is the only way to
    /// branch one into a writable active conversation.
    ReadOnlyArchive,
    /// The session id no longer resolves to a live session.
    MissingSession,
}

impl FlowChatSubmitResult {
    /// True when the message was committed to the transcript (dispatched or
    /// failed closed) and the composer draft should therefore clear.
    pub(crate) fn consumes_draft(self) -> bool {
        matches!(self, Self::Dispatched | Self::FailedClosed)
    }
}

/// What the Flow Desk ⌘K dialog acts on. Derived fresh from view state at
/// toggle/execute time so the popup never captures a stale row.
#[derive(Clone)]
pub(crate) enum FlowDeskSubject {
    /// A flow identity row (or the desk's flow list generally).
    Flow(crate::flows::model::FlowDescriptor),
    /// A conversation session — selected row or the open session view.
    ///
    /// Carries the current descriptor facts so the pure Actions builder can
    /// expose active versus archived commands without reading app state.
    Session {
        id: u64,
        facts: crate::components::conversation_actions::FlowConversationCommandFacts,
        archives: Vec<(String, usize)>,
        /// True only when the subject came from a desk row. An already-open
        /// session must not advertise a redundant "Open Conversation" action.
        open_required: bool,
    },
    /// A background registry run by local id.
    Run(u64),
    /// A setup/recovery row with the same descriptor used by its footer and
    /// Enter path. Only typed, privacy-safe failure metadata may accompany it.
    Recovery {
        descriptor: FlowDeskRowDescriptor,
        failure: Option<crate::ai::reliability::AppFailureRecord>,
    },
    /// The Create Flow affordance.
    Create,
}

#[derive(Clone, Debug)]
struct FlowInputReturnState {
    value: String,
    selection: std::ops::Range<usize>,
    focused_input: FocusedInput,
    pending_focus: Option<FocusTarget>,
}

#[derive(Clone, Debug)]
pub(crate) struct FlowDeskReturnState {
    view: AppView,
    selected_semantic_id: Option<String>,
    input: FlowInputReturnState,
}

#[derive(Clone, Debug)]
pub(crate) struct FlowMainReturnState {
    view: AppView,
    raw_filter_text: String,
    computed_filter_text: String,
    interaction: MainMenuInteractionSnapshot,
    input: FlowInputReturnState,
    pending_placeholder: Option<String>,
}

/// Ephemeral origin captured when a conversation is opened. It deliberately
/// does not enter C05 persistence: returning to a view is presentation state,
/// not conversation history.
#[expect(
    clippy::large_enum_variant,
    reason = "A single return route owns its view state inline without allocating on navigation."
)]
#[derive(Clone, Debug)]
pub(crate) enum FlowConversationReturnRoute {
    Desk(FlowDeskReturnState),
    Main(FlowMainReturnState),
    Direct,
}

fn flow_conversation_return_route_kind(route: &FlowConversationReturnRoute) -> &'static str {
    match route {
        FlowConversationReturnRoute::Desk(_) => "desk",
        FlowConversationReturnRoute::Main(_) => "main",
        FlowConversationReturnRoute::Direct => "direct",
    }
}

fn mdflow_run_accepted_context(phase: crate::flows::model::RunPhase) -> bool {
    matches!(
        phase,
        crate::flows::model::RunPhase::Running
            | crate::flows::model::RunPhase::Succeeded
            | crate::flows::model::RunPhase::Cancelled
    )
}

fn run_phase_icon(phase: crate::flows::model::RunPhase) -> &'static str {
    use crate::flows::model::RunPhase;
    match phase {
        RunPhase::Starting => "◌",
        RunPhase::Running => "●",
        RunPhase::Cancelling => "◍",
        RunPhase::Succeeded => "✓",
        RunPhase::Failed => "✕",
        RunPhase::Cancelled => "⊘",
    }
}

pub(crate) fn resolve_flow_desk_state(
    mdflow_available: bool,
    roster_status: crate::flows::catalog::RosterStatus,
    roster_failure: Option<crate::ai::reliability::AppFailureRecord>,
    query_present: bool,
    matching_row_count: usize,
) -> FlowDeskState {
    use crate::flows::catalog::RosterStatus;

    if !mdflow_available {
        return FlowDeskState::MdflowMissing;
    }
    match roster_status {
        RosterStatus::Loading => FlowDeskState::Loading,
        RosterStatus::Legacy => FlowDeskState::MdflowIncompatible,
        RosterStatus::Error => FlowDeskState::RosterFailed {
            failure: roster_failure.unwrap_or_else(|| {
                crate::ai::reliability::process_failure(
                    sk_protocol::ai_reliability::ProtocolComponent::Mdflow,
                    crate::ai::reliability::ProcessFailureFacts::RuntimeClosed,
                )
            }),
        },
        RosterStatus::Ready if query_present && matching_row_count == 0 => FlowDeskState::NoMatch,
        RosterStatus::Ready if matching_row_count == 0 => FlowDeskState::ReadyEmpty,
        RosterStatus::Ready => FlowDeskState::Ready,
    }
}

pub(crate) fn flow_desk_flow_row_descriptor(
    flow: &crate::flows::model::FlowDescriptor,
) -> FlowDeskRowDescriptor {
    let purpose = flow
        .description
        .clone()
        .unwrap_or_else(|| flow.name.clone());
    let (primary, secondary) = if flow.interactive {
        (FlowDeskRowVerb::OpenInTerminal, None)
    } else if flow.is_workflow {
        (FlowDeskRowVerb::RunOnce, None)
    } else {
        (FlowDeskRowVerb::Converse, Some(FlowDeskRowVerb::RunOnce))
    };
    FlowDeskRowDescriptor {
        semantic_id: format!("flow-desk:flow:{}", flow.id),
        title: flow.friendly_name(),
        detail: format!("{purpose} · {} · {}", flow.engine, flow.origin_label()),
        icon: if flow.interactive {
            "🖥"
        } else if flow.is_workflow {
            "🧩"
        } else {
            "⚡"
        },
        primary,
        secondary,
        actions_available: true,
    }
}

fn format_run_elapsed(ms: u64) -> String {
    let secs = ms / 1000;
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m{:02}s", secs / 60, secs % 60)
    } else {
        format!("{}h{:02}m", secs / 3600, (secs % 3600) / 60)
    }
}

/// Single-quote a path for the Quick Terminal command line (paths with
/// spaces are common under ~/Library and project dirs).
fn shell_escape_path(path: &str) -> String {
    if path
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '-' | '_'))
    {
        path.to_string()
    } else {
        format!("'{}'", path.replace('\'', r"'\''"))
    }
}

/// Safe, redacted diagnostic text for the CopyDetails action: stable code +
/// category + fingerprint only — never raw provider payloads or stderr.
fn flow_recovery_copy_details(meta: &crate::flows::session::FlowSessionMeta) -> String {
    let failure = meta
        .turns
        .iter()
        .rev()
        .find_map(|turn| turn.failure.as_ref());
    match failure {
        Some(failure) => format!(
            "Flow: {}\nEngine: {}\nFailure code: {:?}\nCategory: {:?}\nSummary: {}\nDiagnostic fingerprint: {}",
            meta.flow_id,
            meta.engine,
            failure.code,
            failure.category,
            failure.safe_summary,
            failure
                .diagnostic_fingerprint
                .as_deref()
                .unwrap_or("unavailable"),
        ),
        None => format!(
            "Flow: {}\nEngine: {}\nNo settled failure recorded for this session.",
            meta.flow_id, meta.engine
        ),
    }
}

impl ScriptListApp {
    /// Effective cwd for flow discovery: the spine cwd chip when set,
    /// otherwise $HOME. mdflow resolves project vs global flows from here.
    pub(crate) fn flow_ux_cwd(&self) -> String {
        if let Some(scope) = crate::runtime_policy::owned_evaluation() {
            return scope.root().to_string_lossy().into_owned();
        }
        crate::flows::resolve_flow_cwd(
            self.spine_cwd
                .as_ref()
                .map(|cwd| cwd.to_string_lossy().to_string()),
        )
    }

    /// Short human form of the flow cwd for chips/empty states: `~`-relative
    /// when under $HOME, and never more than the last two components.
    fn flow_ux_cwd_display(cwd: &str) -> String {
        let home = std::env::var("HOME").unwrap_or_default();
        if !home.is_empty() && cwd == home {
            return "~".to_string();
        }
        let tail: Vec<&str> = cwd.rsplit('/').filter(|s| !s.is_empty()).take(2).collect();
        match tail.as_slice() {
            [last, parent] => format!("{parent}/{last}"),
            [last] => (*last).to_string(),
            _ => cwd.to_string(),
        }
    }

    /// Spawn the repaint tick that keeps flow surfaces live. Single
    /// instance; exits when nothing is active. The tick is the ONLY seam
    /// where transport events reach GPUI entities: codex app-server events
    /// and mdflow run tails apply here on the main thread every 120ms.
    /// (ChatPrompt callback requests drain in the render pass instead —
    /// they need window access.)
    pub(crate) fn start_flow_ux_tick(&mut self, cx: &mut Context<Self>) {
        // Owned sources deliver to the same reducers from bounded evaluator
        // events; they never poll codex or mdflow's process registries.
        if crate::runtime_policy::is_owned_evaluation() {
            return;
        }
        if self.flow_ux_tick_running {
            return;
        }
        self.flow_ux_tick_running = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(120))
                    .await;
                // WP5: one 8 Hz tick wake. WP9 wants this to stop after a
                // session settles; the throttled snapshot also gives the
                // quiet-idle probe phase a steady reading to sample.
                crate::chat_hot_counters::record_flow_tick_wake();
                crate::chat_hot_counters::maybe_log_snapshot("flow_tick");
                let keep_going = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        let registry = crate::flows::run_registry::flow_run_registry();
                        let generation = registry.generation();
                        let mut dirty = generation != app.flow_ux_seen_generation;
                        app.flow_ux_seen_generation = generation;

                        // ChatPrompt callback requests are drained in the
                        // app render pass (render_impl — needs window
                        // access); this tick owns transport events only.
                        // While a session view is open, repaint every tick
                        // so that drain is guaranteed to run within 120ms
                        // of a callback posting a request.
                        if matches!(app.current_view, AppView::FlowSessionView { .. }) {
                            dirty = true;
                        }

                        // 1. Codex app-server events (native transport).
                        for event in crate::flows::codex_client::codex_app_server().drain_events() {
                            dirty = true;
                            app.apply_flow_thread_event(event, cx);
                        }

                        // 2. mdflow-turn runs: stream stdout, settle turns.
                        if app.sync_mdflow_turns(cx) {
                            dirty = true;
                        }

                        // 3. Bare runs (run-once / workflows) that reached a
                        // terminal phase get exactly one receipt toast —
                        // silence must never look identical to success.
                        for run in registry.take_unnotified_terminal() {
                            use crate::flows::model::RunPhase;
                            let friendly = crate::flows::model::friendly_flow_name(&run.flow_name);
                            let elapsed = format_run_elapsed(run.elapsed_ms());
                            let toast = match run.phase {
                                RunPhase::Succeeded => crate::components::toast::Toast::success(
                                    format!("{friendly} finished ({elapsed})"),
                                    &app.theme,
                                ),
                                RunPhase::Cancelled => crate::components::toast::Toast::success(
                                    format!("{friendly} cancelled ({elapsed})"),
                                    &app.theme,
                                ),
                                _ => crate::components::toast::Toast::error(
                                    format!("{friendly}: {}", run.display_status()),
                                    &app.theme,
                                ),
                            };
                            app.toast_manager.push(toast.duration_ms(Some(4000)));
                            dirty = true;
                        }

                        if dirty {
                            // WP5/C-R8: a tick that requested a root
                            // invalidation via `cx.notify()`. This is NOT a
                            // render — flow-session ticks force `dirty` every
                            // wake, so this rises even when nothing paints. The
                            // actual repaint is counted by the split
                            // desk/session render counters at the top of the
                            // real Flow render functions.
                            crate::chat_hot_counters::record_flow_render_request();
                            cx.notify();
                        }
                        let view_active = matches!(
                            app.current_view,
                            AppView::FlowUxView { .. } | AppView::FlowSessionView { .. }
                        );
                        // Idle sessions must NOT pin this loop forever (an
                        // 8 Hz wake-up for a backgrounded conversation is
                        // pure battery drain — 2026-07-11 audit). Sessions
                        // only keep the tick alive while a turn is in
                        // flight; submitting restarts the tick.
                        let any_turn_in_flight = app
                            .conversations
                            .flow_sessions
                            .iter()
                            .any(|(meta, _)| meta.active_turn.is_some());
                        let keep = view_active || registry.active_count() > 0 || any_turn_in_flight;
                        if !keep {
                            app.flow_ux_tick_running = false;
                        }
                        keep
                    })
                });
                match keep_going {
                    Ok(true) => continue,
                    _ => break,
                }
            }
        })
        .detach();
    }

    // ------------------------------------------------------------------
    // Desk corpus + rows
    // ------------------------------------------------------------------

    /// The combined flow corpus for the desk (roster for the effective cwd
    /// plus the installed flows package).
    pub(crate) fn flow_desk_corpus(&self) -> Vec<crate::flows::model::FlowDescriptor> {
        let cwd = self.flow_ux_cwd();
        let roster = crate::flows::catalog::flow_catalog().roster_for(&cwd);
        crate::flows::catalog::desk_flows(&roster)
    }

    /// Real content rows only: setup and utility affordances are added later
    /// from the typed [`FlowDeskState`].
    fn flow_desk_content_rows(&self, filter: &str) -> Vec<FlowDeskRow> {
        let mut rows: Vec<FlowDeskRow> = Vec::new();
        let query = filter.trim().to_lowercase();

        let mut sessions: Vec<&crate::flows::session::FlowSessionMeta> = self
            .conversations
            .flow_sessions
            .iter()
            .map(|(meta, _)| meta)
            .collect();
        sessions.sort_by_key(|a| std::cmp::Reverse(a.id));
        for meta in sessions {
            let matches = query.is_empty()
                || meta.friendly_name.to_lowercase().contains(&query)
                || meta.flow_name.to_lowercase().contains(&query);
            if matches {
                rows.push(FlowDeskRow::Session(meta.id));
            }
        }

        let registry = crate::flows::run_registry::flow_run_registry();
        let mut runs: Vec<crate::flows::run_registry::RunSummary> = registry
            .run_summaries()
            .into_iter()
            .filter(|run| !run.is_conversation)
            .filter(|run| {
                query.is_empty()
                    || run.flow_name.to_lowercase().contains(&query)
                    || crate::flows::model::friendly_flow_name(&run.flow_name)
                        .to_lowercase()
                        .contains(&query)
            })
            .collect();
        runs.sort_by(|a, b| {
            let a_active = !a.phase.is_terminal();
            let b_active = !b.phase.is_terminal();
            b_active
                .cmp(&a_active)
                .then_with(|| b.local_id.cmp(&a.local_id))
        });
        rows.extend(runs.into_iter().map(|run| FlowDeskRow::Run(run.local_id)));

        let corpus = self.flow_desk_corpus();
        rows.extend(
            crate::flows::catalog::filter_flows(&corpus, filter)
                .into_iter()
                .cloned()
                .map(|flow| FlowDeskRow::Flow(Box::new(flow))),
        );
        rows
    }

    pub(crate) fn flow_desk_state(&self, filter: &str) -> FlowDeskState {
        let matching_row_count = self.flow_desk_content_rows(filter).len();
        let cwd = self.flow_ux_cwd();
        let roster = crate::flows::catalog::flow_catalog().roster_for(&cwd);
        resolve_flow_desk_state(
            crate::runtime_policy::is_owned_evaluation()
                || crate::flows::catalog::mdflow_binary().is_some(),
            roster.status,
            roster.failure,
            !filter.trim().is_empty(),
            matching_row_count,
        )
    }

    /// Build selectable rows from real content plus the exact recovery owned by
    /// the typed desk state. Degraded setup never hides resumable content.
    pub(crate) fn flow_desk_rows(&self, filter: &str) -> Vec<FlowDeskRow> {
        let mut rows = self.flow_desk_content_rows(filter);
        match self.flow_desk_state(filter) {
            FlowDeskState::Loading => {}
            FlowDeskState::MdflowMissing => rows.push(FlowDeskRow::InstallMdflow),
            FlowDeskState::MdflowIncompatible => rows.push(FlowDeskRow::UpgradeMdflow),
            FlowDeskState::RosterFailed { .. } => rows.push(FlowDeskRow::RetryRoster),
            FlowDeskState::ReadyEmpty => {
                rows.push(FlowDeskRow::InitFlows);
                rows.push(FlowDeskRow::CreateFlow);
            }
            FlowDeskState::NoMatch => {
                rows.push(FlowDeskRow::ClearQuery);
                rows.push(FlowDeskRow::CreateFlow);
            }
            FlowDeskState::Ready => rows.push(FlowDeskRow::CreateFlow),
        }
        rows
    }

    pub(crate) fn flow_desk_row_descriptor(&self, row: &FlowDeskRow) -> FlowDeskRowDescriptor {
        match row {
            FlowDeskRow::Session(session_id) => {
                let meta = self
                    .conversations
                    .flow_sessions
                    .iter()
                    .find(|(meta, _)| meta.id == *session_id)
                    .map(|(meta, _)| meta);
                FlowDeskRowDescriptor {
                    semantic_id: format!("flow-desk:session:{session_id}"),
                    title: meta
                        .map(|meta| meta.friendly_name.clone())
                        .unwrap_or_else(|| "Conversation".to_string()),
                    detail: meta
                        .map(|meta| {
                            format!(
                                "{} · {} · {} · conversation",
                                meta.state.label(),
                                meta.elapsed_label(),
                                meta.engine,
                            )
                        })
                        .unwrap_or_else(|| "Conversation unavailable".to_string()),
                    icon: if meta.is_some_and(|meta| meta.state.is_live()) {
                        "💬"
                    } else {
                        "◽"
                    },
                    primary: FlowDeskRowVerb::OpenConversation,
                    secondary: None,
                    actions_available: true,
                }
            }
            FlowDeskRow::Run(run_id) => {
                let run = crate::flows::run_registry::flow_run_registry().get(*run_id);
                FlowDeskRowDescriptor {
                    semantic_id: format!("flow-desk:run:{run_id}"),
                    title: run
                        .as_ref()
                        .map(|run| crate::flows::model::friendly_flow_name(&run.flow_name))
                        .unwrap_or_else(|| "Run".to_string()),
                    detail: run
                        .as_ref()
                        .map(|run| {
                            format!(
                                "{} · {} · {}",
                                run.display_status(),
                                format_run_elapsed(run.elapsed_ms()),
                                run.last_output_line().unwrap_or("—"),
                            )
                        })
                        .unwrap_or_else(|| "Run unavailable".to_string()),
                    icon: run
                        .as_ref()
                        .map(|run| run_phase_icon(run.phase))
                        .unwrap_or("◽"),
                    primary: FlowDeskRowVerb::OpenRunActions,
                    secondary: None,
                    actions_available: true,
                }
            }
            FlowDeskRow::Flow(flow) => flow_desk_flow_row_descriptor(flow),
            FlowDeskRow::InstallMdflow => FlowDeskRowDescriptor {
                semantic_id: "flow-desk:recovery:install-mdflow".to_string(),
                title: "Install mdflow".to_string(),
                detail: "The flow engine isn't on PATH — open the install command in Terminal"
                    .to_string(),
                icon: "⬇",
                primary: FlowDeskRowVerb::InstallMdflow,
                secondary: None,
                actions_available: false,
            },
            FlowDeskRow::UpgradeMdflow => FlowDeskRowDescriptor {
                semantic_id: "flow-desk:recovery:upgrade-mdflow".to_string(),
                title: "Upgrade mdflow".to_string(),
                detail: "The installed mdflow predates the roster protocol".to_string(),
                icon: "↥",
                primary: FlowDeskRowVerb::UpgradeMdflow,
                secondary: None,
                actions_available: false,
            },
            FlowDeskRow::RetryRoster => FlowDeskRowDescriptor {
                semantic_id: "flow-desk:recovery:retry-roster".to_string(),
                title: "Retry flow discovery".to_string(),
                detail: match self.flow_desk_state("") {
                    FlowDeskState::RosterFailed { failure } => {
                        failure.primary_message().to_string()
                    }
                    _ => {
                        "Flow discovery did not finish; retry with your work preserved".to_string()
                    }
                },
                icon: "↻",
                primary: FlowDeskRowVerb::RetryRoster,
                secondary: None,
                actions_available: true,
            },
            FlowDeskRow::ClearQuery => FlowDeskRowDescriptor {
                semantic_id: "flow-desk:recovery:clear-search".to_string(),
                title: "Clear search".to_string(),
                detail: "Show every available conversation, run, and flow".to_string(),
                icon: "⌫",
                primary: FlowDeskRowVerb::ClearSearch,
                secondary: None,
                actions_available: false,
            },
            FlowDeskRow::InitFlows => FlowDeskRowDescriptor {
                semantic_id: "flow-desk:recovery:scaffold".to_string(),
                title: "Scaffold starter flows".to_string(),
                detail: "md init creates a flows/ roster here (no engine calls)".to_string(),
                icon: "🌱",
                primary: FlowDeskRowVerb::ScaffoldFlows,
                secondary: None,
                actions_available: false,
            },
            FlowDeskRow::CreateFlow => FlowDeskRowDescriptor {
                semantic_id: "flow-desk:utility:create".to_string(),
                title: "Create a flow…".to_string(),
                detail: "Describe an agent in plain English (md create)".to_string(),
                icon: "✚",
                primary: FlowDeskRowVerb::CreateFlow,
                secondary: None,
                actions_available: true,
            },
        }
    }
}

include!("flow_ux_session_runtime.rs");
include!("flow_ux_session_navigation.rs");

impl ScriptListApp {
    // ------------------------------------------------------------------
    // Desk render
    // ------------------------------------------------------------------

    fn render_flow_ux(
        &mut self,
        _variant: crate::flows::model::FlowUxVariant,
        filter: String,
        selected_index: usize,
        _inline_run: Option<u64>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        // C-R8/WP-B3: one honest Flow *desk* surface render (on screen and
        // painting). Distinct from tick invalidation requests.
        crate::chat_hot_counters::record_flow_desk_render();
        let chrome = crate::theme::AppChromeColors::from_theme(&self.theme);
        let list_colors = crate::list_item::ListItemColors::from_theme(&self.theme);
        let cwd = self.flow_ux_cwd();
        let desk_state = self.flow_desk_state(&filter);
        let rows = self.flow_desk_rows(&filter);
        let row_count = rows.len();
        let registry = crate::flows::run_registry::flow_run_registry();

        // ------------------------------------------------------------------
        // Key handler — the Conversation Desk grammar.
        // ------------------------------------------------------------------
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                this.hide_mouse_cursor(cx);
                let key = event.keystroke.key.as_str();
                let has_cmd = event.keystroke.modifiers.platform;
                let has_shift = event.keystroke.modifiers.shift;

                let view_state = if let AppView::FlowUxView {
                    filter,
                    selected_index,
                    ..
                } = &this.current_view
                {
                    Some((filter.clone(), *selected_index))
                } else {
                    None
                };
                let Some((current_filter, current_selected)) = view_state else {
                    return;
                };

                if is_key_escape(key) && !this.show_actions_popup {
                    if !this.clear_builtin_view_filter(cx) {
                        this.go_back_or_close(window, cx);
                    }
                    cx.stop_propagation();
                    return;
                }

                if has_cmd && key.eq_ignore_ascii_case("w") {
                    this.close_and_reset_window(cx);
                    cx.stop_propagation();
                    return;
                }

                let rows = this.flow_desk_rows(&current_filter);
                let current_len = rows.len();

                if is_key_up(key) {
                    if current_selected > 0 {
                        if let AppView::FlowUxView { selected_index, .. } = &mut this.current_view {
                            *selected_index = current_selected - 1;
                            this.flow_ux_scroll_handle
                                .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                        }
                        cx.notify();
                    }
                    cx.stop_propagation();
                    return;
                }
                if is_key_down(key) {
                    if current_selected < current_len.saturating_sub(1) {
                        if let AppView::FlowUxView { selected_index, .. } = &mut this.current_view {
                            *selected_index = current_selected + 1;
                            this.flow_ux_scroll_handle
                                .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                        }
                        cx.notify();
                    }
                    cx.stop_propagation();
                    return;
                }

                if is_key_enter(key) {
                    this.flow_desk_activate_selected(has_shift, window, cx);
                    cx.stop_propagation();
                }
            },
        );

        // ------------------------------------------------------------------
        // List element
        // ------------------------------------------------------------------
        let cwd_display = Self::flow_ux_cwd_display(&cwd);
        let empty_message = match &desk_state {
            FlowDeskState::Loading => format!("Loading flows in {cwd_display}…"),
            FlowDeskState::MdflowMissing => {
                "mdflow is required before flows can be discovered".to_string()
            }
            FlowDeskState::MdflowIncompatible => {
                "The installed mdflow needs an update for Flow Desk".to_string()
            }
            FlowDeskState::RosterFailed { failure } => failure.primary_message().to_string(),
            FlowDeskState::ReadyEmpty => {
                format!("No flows are configured in {cwd_display} yet")
            }
            FlowDeskState::NoMatch => "No conversations, runs, or flows match".to_string(),
            FlowDeskState::Ready => String::new(),
        };

        let list_element: gpui::AnyElement = {
            let display_rows: Vec<FlowDeskRowDescriptor> = rows
                .iter()
                .map(|row| self.flow_desk_row_descriptor(row))
                .collect();
            let hovered = self.hovered_index;
            let click_entity = cx.entity();
            uniform_list(
                "flow-desk-list",
                row_count,
                move |visible_range, _window, _cx| {
                    visible_range
                        .map(|ix| {
                            let is_selected = ix == selected_index;
                            let is_hovered = hovered == Some(ix);
                            let descriptor = &display_rows[ix];
                            let row_entity = click_entity.clone();
                            div()
                                .id(ix)
                                .cursor_pointer()
                                .on_mouse_down(
                                    gpui::MouseButton::Left,
                                    move |_event, window, cx| {
                                        row_entity.update(cx, |app, cx| {
                                            app.flow_desk_click_row(ix, window, cx);
                                        });
                                    },
                                )
                                .child(
                                    ListItem::new(descriptor.title.clone(), list_colors)
                                        .description_opt(Some(descriptor.detail.clone()))
                                        .icon(descriptor.icon)
                                        .selected(is_selected)
                                        .hovered(is_hovered),
                                )
                        })
                        .collect()
                },
            )
            .h_full()
            .track_scroll(&self.flow_ux_scroll_handle)
            .into_any_element()
        };

        let list_scrollbar =
            self.builtin_uniform_list_scrollbar(&self.flow_ux_scroll_handle, row_count, 8);
        // Every list leads with a persistent section separator (POLISH.md
        // layout-stability bar; same rule as the main menu's "Results"
        // header, 4d76327b8): the label may swap but the row never appears
        // or disappears, so filtering can't shift the rows below it.
        let leading_header = crate::list_item::render_section_header(
            if filter.trim().is_empty() {
                "Flows"
            } else {
                "Results"
            },
            None,
            list_colors,
            true,
        );
        let mut list_pane = div()
            .relative()
            .w_full()
            .h_full()
            .min_h(px(0.))
            .flex()
            .flex_col()
            .child(leading_header)
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h(px(0.))
                    .child(list_element)
                    .child(list_scrollbar),
            );
        if !empty_message.is_empty() {
            // Roster problems surface as a banner above the (package) rows
            // instead of replacing the whole list — package flows still work
            // when a repo has none of its own.
            list_pane = div()
                .relative()
                .w_full()
                .h_full()
                .min_h(px(0.))
                .flex()
                .flex_col()
                .child(
                    div()
                        .px_3()
                        .py_1()
                        .text_xs()
                        .text_color(rgb(chrome.text_muted_hex))
                        .child(empty_message),
                )
                .child(div().flex_1().min_h(px(0.)).child(list_pane));
        }
        let main = list_pane.into_any_element();

        // ------------------------------------------------------------------
        // Footer + shell (Conversation Desk contract: primary verbs only).
        // ------------------------------------------------------------------
        let live_sessions = self
            .conversations
            .flow_sessions
            .iter()
            .filter(|(meta, _)| meta.state.is_live())
            .count();
        let active_runs = registry.active_count();
        let selected_descriptor = rows
            .get(selected_index)
            .map(|row| self.flow_desk_row_descriptor(row));
        let mut hints: Vec<gpui::SharedString> = Vec::new();
        if let Some(descriptor) = &selected_descriptor {
            hints.push(gpui::SharedString::from(format!(
                "↵ {}",
                descriptor.primary.label()
            )));
            if let Some(secondary) = descriptor.secondary {
                hints.push(gpui::SharedString::from(format!(
                    "⇧↵ {}",
                    secondary.label()
                )));
            }
            if descriptor.actions_available {
                hints.push(gpui::SharedString::from("⌘K Actions"));
            }
        }
        hints.push(gpui::SharedString::from("Esc Back"));
        let footer =
            self.main_window_footer_slot(crate::components::render_simple_hint_strip(hints, None));

        let mut count_parts: Vec<String> = Vec::new();
        if live_sessions > 0 {
            count_parts.push(format!("{live_sessions} active"));
        }
        if active_runs > 0 {
            count_parts.push(format!("{active_runs} running"));
        }
        // Count FLOW rows only — sessions, runs, and affordance rows are not
        // flows (the old `row_count - 1` reported "7 flows" for 5 flows +
        // 2 sessions).
        let flow_row_count = rows
            .iter()
            .filter(|row| matches!(row, FlowDeskRow::Flow(_)))
            .count();
        count_parts.push(format!("{flow_row_count} flows"));
        let count_label = count_parts.join(" · ");
        // Trailing slot = the standard muted count label only. The flow cwd
        // already shows in the shared context zone (top-left chip) — never
        // duplicate it beside the input.
        let trailing = vec![self.render_builtin_main_input_count_label(count_label)];

        let menu_def = self.current_main_menu_theme.def();
        let shell = menu_def.shell;
        crate::components::main_view_chrome::render_main_view_chrome_footer_flush(
            crate::components::main_view_chrome::render_main_view_shell()
                .text_color(rgb(chrome.text_primary_hex))
                .font_family(self.theme_font_family())
                .key_context("flow_ux")
                .track_focus(&self.focus_handle)
                .on_key_down(handle_key),
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewChrome {
                header: self.render_builtin_main_input_header(trailing, cx),
                divider: crate::components::main_view_chrome::MainViewDividerChrome {
                    margin_x: shell.divider_margin_x,
                    height: shell.divider_height,
                    visible: shell.divider_height > 0.0,
                },
                main,
                footer,
                overlays: Vec::new(),
            },
        )
    }

    // ------------------------------------------------------------------
    // Session render (FlowSessionView)
    // ------------------------------------------------------------------

    pub(crate) fn render_flow_session(
        &mut self,
        session_id: u64,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        // C-R8/WP-B3: one honest Flow *session* surface render (the session
        // transcript is on screen and painting). Distinct from tick requests.
        crate::chat_hot_counters::record_flow_session_render();
        let chrome = crate::theme::AppChromeColors::from_theme(&self.theme);
        let Some(index) = self.flow_session_index(session_id) else {
            // Session vanished (dismissed elsewhere) — fall back to the desk.
            return self.render_flow_ux(
                crate::flows::model::FlowUxVariant::Flash,
                String::new(),
                0,
                None,
                cx,
            );
        };
        // WP-B3: one session scanned + the per-render `FlowSessionMeta` clone.
        // The clone is O(turns) and happens every session render; count it so
        // WP9 can see the per-render allocation cost (the borrow checker forces
        // it here because `entity` and `meta` both borrow `self.conversations.flow_sessions`).
        crate::chat_hot_counters::record_flow_session_scanned();
        let (meta, entity) = {
            let (meta, entity) = &self.conversations.flow_sessions[index];
            (meta.clone(), entity.clone())
        };

        // The MAIN input is the composer — the same shared input every
        // surface uses (with its context-attachment features). Identity
        // lives where every surface puts it: the placeholder names the flow
        // and the shared context zone's Agent·Model chip carries
        // flow · engine (see `main_view_context_labels`). The input row's
        // trailing slot stays empty — it is a count-label slot on list
        // surfaces, never a status bar.
        let trailing: Vec<gpui::AnyElement> = Vec::new();

        // Single exhaustive key owner (C-R1): WP7 removed ChatPrompt's own key
        // handling for this transcript-only host, so every non-destructive
        // binding — Background, Stop, ToggleActions, Submit — resolves here and exactly
        // ONE action runs per press. Plain ↵ submits; Shift/Cmd+Enter fall
        // through to the composer (never silently submit); ⌘. cancels the
        // in-flight turn without backgrounding or terminating.
        let viewing_archive = meta.selected_is_archived();
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                let AppView::FlowSessionView { session_id } = this.current_view else {
                    return;
                };
                let key = event.keystroke.key.as_str();
                let platform = event.keystroke.modifiers.platform;
                let shift = event.keystroke.modifiers.shift;
                let command_facts = this.flow_conversation_command_facts(session_id);
                let actions_open = this.show_actions_popup;
                if platform && key.eq_ignore_ascii_case("w") && !actions_open {
                    this.capture_flow_session_draft(session_id);
                    this.close_and_reset_window(cx);
                    cx.stop_propagation();
                    return;
                }
                if viewing_archive
                    && !actions_open
                    && !platform
                    && !shift
                    && crate::ui_foundation::is_key_escape(key)
                {
                    this.show_current_flow_conversation(session_id, cx);
                    cx.stop_propagation();
                    return;
                }

                match resolve_flow_session_key_action(
                    key,
                    platform,
                    shift,
                    command_facts,
                    actions_open,
                ) {
                    FlowSessionKeyAction::Background => {
                        this.background_flow_session(window, cx);
                        cx.stop_propagation();
                    }
                    FlowSessionKeyAction::Stop => {
                        // ⌘. cancels the in-flight turn only; the conversation
                        // survives and the composer stays usable.
                        this.stop_flow_session(session_id, cx);
                        cx.stop_propagation();
                    }
                    FlowSessionKeyAction::ToggleActions => {
                        this.dispatch_actions_toggle_for_current_view(
                            window,
                            cx,
                            "flow_session_chat",
                        );
                        cx.stop_propagation();
                    }
                    FlowSessionKeyAction::NewConversation => {
                        // Refusal while working is the handler's job and it
                        // toasts for itself, so the key is consumed either
                        // way — ⌘L must never fall through and type an "l"
                        // into the composer.
                        this.start_fresh_flow_conversation(
                            session_id,
                            crate::flows::session::FlowConversationResetCause::UserRequested,
                            cx,
                        );
                        cx.stop_propagation();
                    }
                    FlowSessionKeyAction::CopyLastResponse => {
                        // Consumed either way. The empty-transcript case toasts
                        // for itself inside the shared transaction, so falling
                        // through here would type a "C" into the composer on
                        // top of an already-answered chord.
                        this.copy_flow_session_last_response(session_id, cx);
                        cx.stop_propagation();
                    }
                    FlowSessionKeyAction::Submit => {
                        if !viewing_archive {
                            // One shared draft transaction: clears the composer ONLY
                            // when the submit consumed the draft (WP1 P0: clearing
                            // before submit destroyed the message on a Busy race).
                            let _ = this.submit_flow_session_draft(session_id, window, cx);
                            cx.stop_propagation();
                        }
                    }
                    FlowSessionKeyAction::Ignore => {}
                }
            },
        );

        // Honest state rides as the footer hint strip's leading status text —
        // the same slot ChatPrompt's own footer uses ("Streaming · model").
        // No ticking elapsed timer in chrome; the desk row carries elapsed.
        let status_text = if meta.selected_is_archived() {
            format!(
                "Archived · {} turns · read-only",
                meta.selected_turns().len()
            )
        } else if meta.active_turn.is_some() && !meta.thread_ready {
            format!("Connecting · {}", meta.engine)
        } else if meta.active_turn.is_some() {
            format!("Working · {}", meta.engine)
        } else {
            format!("Active · {}", meta.engine)
        };
        // Truthful footer (WP1): a busy/connecting session cannot accept a
        // submit, so it must NOT advertise `↵ Send`; the leading status text
        // already carries "Working/Connecting". The pure `flow_session_footer_hints`
        // helper owns this rule so it can be unit-tested without a window.
        let hints = if meta.selected_is_archived() {
            vec![
                gpui::SharedString::from("Esc Back to Current"),
                gpui::SharedString::from("⌘K Actions"),
            ]
        } else {
            flow_session_footer_hints(meta.active_turn.is_some())
        };
        let footer = self.main_window_footer_slot(crate::components::render_simple_hint_strip(
            hints,
            Some(crate::components::render_hint_strip_leading_text(
                status_text,
                self.theme.colors.text.primary,
            )),
        ));

        let menu_def = self.current_main_menu_theme.def();
        let shell = menu_def.shell;
        let header = if meta.selected_is_archived() {
            crate::components::main_view_chrome::MainViewHeaderChrome::context_only(
                menu_def,
                self.render_clickable_main_view_context_zone(menu_def, cx),
            )
        } else {
            self.render_builtin_main_input_header(trailing, cx)
        };
        crate::components::main_view_chrome::render_main_view_chrome_footer_flush(
            crate::components::main_view_chrome::render_main_view_shell()
                .text_color(rgb(chrome.text_primary_hex))
                .font_family(self.theme_font_family())
                .key_context("flow_session")
                .track_focus(&self.focus_handle)
                .on_key_down(handle_key),
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewChrome {
                header,
                divider: crate::components::main_view_chrome::MainViewDividerChrome {
                    margin_x: shell.divider_margin_x,
                    height: shell.divider_height,
                    visible: shell.divider_height > 0.0,
                },
                main: {
                    // S09: the shared AI recovery card (same anatomy and
                    // `ai-recovery-*` semantic ids as Agent Chat/Quick AI)
                    // projects from the reducer-owned session state. It
                    // renders below the transcript in the TranscriptCard
                    // layout; a settled Ok/Stopped turn projects nothing.
                    let recovery_card = (!meta.selected_is_archived())
                        .then(|| {
                            crate::ai::reliability::project_recovery(
                                &meta.reliability.state().identity,
                                meta.reliability.state(),
                                &crate::ai::reliability::flow_session_recovery_capabilities(),
                            )
                        })
                        .flatten();
                    let mut main = div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h(px(0.))
                        .w_full()
                        .child(entity);
                    if let Some(spec) = recovery_card {
                        let weak = cx.entity().downgrade();
                        let action_weak = weak.clone();
                        let dismiss_weak = weak;
                        let handlers = crate::components::AiRecoveryCardHandlers {
                            on_action: std::rc::Rc::new(move |action, window, cx| {
                                if let Some(entity) = action_weak.upgrade() {
                                    entity.update(cx, |this: &mut Self, cx| {
                                        this.dispatch_flow_recovery_action(
                                            session_id, action, window, cx,
                                        );
                                    });
                                }
                            }),
                            on_dismiss: Some(std::rc::Rc::new(move |_window, cx| {
                                if let Some(entity) = dismiss_weak.upgrade() {
                                    entity.update(cx, |this: &mut Self, cx| {
                                        this.dismiss_flow_recovery(session_id, cx);
                                    });
                                }
                            })),
                        };
                        // The card is the message. Its actions live in the
                        // shared footer rail below it, never as loose buttons
                        // inside the session transcript.
                        let plan = crate::ai::reliability::plan_recovery_presentation(&spec);
                        let recovery_footer =
                            crate::components::render_ai_recovery_footer(&plan, &handlers, None);
                        main = main.child(
                            div()
                                .id("flow-session-recovery-stack")
                                .w_full()
                                .px(px(12.0))
                                .pb(px(6.0))
                                .child(crate::components::render_ai_recovery_card(
                                    spec,
                                    &self.theme,
                                ))
                                .children(recovery_footer),
                        );
                    }
                    main.into_any_element()
                },
                footer,
                overlays: Vec::new(),
            },
        )
    }

    /// Route one shared-card recovery action back into the owning Flow
    /// session (S10 contract: ChatPrompt never invents Flow retry behavior —
    /// the Flow surface owns dispatch).
    pub(crate) fn dispatch_flow_recovery_action(
        &mut self,
        session_id: u64,
        action: sk_protocol::ai_reliability::AiRecoveryAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use sk_protocol::ai_reliability::AiRecoveryAction;
        let Some(index) = self.flow_session_index(session_id) else {
            return;
        };
        tracing::info!(
            target: "script_kit::flows",
            event = "flow_recovery_action",
            session_id,
            action = ?action,
            "Flow recovery action selected"
        );
        match action {
            AiRecoveryAction::Retry => {
                self.retry_flow_turn(session_id, cx);
            }
            AiRecoveryAction::RethreadFlow => {
                if !self.conversations.flow_sessions[index]
                    .0
                    .reliability
                    .select_rethread()
                {
                    return;
                }
                // A rethread lands the next submit on a FRESH protocol
                // thread carrying the flow contract + transcript rollup —
                // the SAME transaction "New Conversation" uses, with the
                // cause that preserves the transcript.
                if !self.start_fresh_flow_conversation(
                    session_id,
                    crate::flows::session::FlowConversationResetCause::Recovery,
                    cx,
                ) {
                    return;
                }
                self.retry_flow_turn(session_id, cx);
            }
            AiRecoveryAction::RepairComponent { .. } => {
                if crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Process)
                    .is_err()
                {
                    return;
                }
                // mdflow missing/broken: the desk's install affordance is
                // the one repair path (quick terminal `npm i -g mdflow`).
                self.conversations.flow_sessions[index]
                    .0
                    .reliability
                    .select_recovery(AiRecoveryAction::RepairComponent {
                        component: sk_protocol::ai_reliability::ProtocolComponent::Mdflow,
                    });
                let _ = window;
                self.open_quick_terminal_with_command(None, "npm i -g mdflow".to_string(), cx);
            }
            AiRecoveryAction::CopyDetails => {
                let meta = &self.conversations.flow_sessions[index].0;
                let details = flow_recovery_copy_details(meta);
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(details));
                self.toast_manager.push(
                    crate::components::toast::Toast::success(
                        "Safe diagnostic details copied".to_string(),
                        &self.theme,
                    )
                    .duration_ms(Some(2000)),
                );
                cx.notify();
            }
            _ => {
                tracing::warn!(
                    target: "script_kit::flows",
                    event = "flow_recovery_action_unsupported",
                    session_id,
                    action = ?action,
                    "Flow surface does not dispatch this recovery action"
                );
            }
        }
    }

    pub(crate) fn dismiss_flow_recovery(&mut self, session_id: u64, cx: &mut Context<Self>) {
        let Some(index) = self.flow_session_index(session_id) else {
            return;
        };
        self.conversations.flow_sessions[index]
            .0
            .reliability
            .dismiss();
        cx.notify();
    }

    /// Retry the failed turn WITHOUT duplicating its user row: the failed
    /// turn stays in the transcript (typed error row), a fresh streaming
    /// bubble carries the retried attempt of the same user text.
    fn retry_flow_turn(&mut self, session_id: u64, cx: &mut Context<Self>) {
        let Some(index) = self.flow_session_index(session_id) else {
            return;
        };
        if self.conversations.flow_sessions[index]
            .0
            .active_turn
            .is_some()
        {
            return;
        }
        let Some(user_text) = self.conversations.flow_sessions[index]
            .0
            .turns
            .iter()
            .rev()
            .find(|turn| turn.outcome == crate::flows::session::PersistedTurnOutcome::Failed)
            .map(|turn| turn.user.clone())
        else {
            return;
        };
        let turn_ordinal = self.conversations.flow_sessions[index].0.turns.len();
        let flow_id = self.conversations.flow_sessions[index].0.flow_id.clone();
        if !self.conversations.flow_sessions[index]
            .0
            .reliability
            .retry_turn(&flow_id, turn_ordinal)
        {
            self.toast_manager.push(
                crate::components::toast::Toast::error(
                    "Retry limit reached — start a new thread from ⌘K".to_string(),
                    &self.theme,
                )
                .duration_ms(Some(2500)),
            );
            cx.notify();
            return;
        }
        self.dispatch_flow_turn_without_user_echo(session_id, user_text, cx);
    }

    /// Dispatch one turn on the session's transport with the streaming
    /// bubble only (no user-row echo — used by Retry, where the user text is
    /// already in the transcript on the failed turn).
    fn dispatch_flow_turn_without_user_echo(
        &mut self,
        session_id: u64,
        text: String,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.flow_session_index(session_id) else {
            return;
        };
        let mut thread_profile: Option<crate::flows::session::FlowThreadProfile> = None;
        let (transport, prompt) = {
            let meta = &self.conversations.flow_sessions[index].0;
            let prompt = match meta.transport {
                crate::flows::session::SessionTransport::CodexThread => {
                    if meta.turns.is_empty() || meta.needs_rethread {
                        match std::fs::read_to_string(&meta.flow_path) {
                            Ok(markdown) => {
                                let task = if meta.turns.is_empty() {
                                    text.clone()
                                } else {
                                    crate::flows::session::build_turn_task(&meta.turns, &text)
                                };
                                let contract = crate::flows::session::resolve_flow_thread_contract(
                                    &markdown, &task,
                                );
                                thread_profile = Some(contract.profile);
                                contract.first_prompt
                            }
                            Err(_) => text.clone(),
                        }
                    } else {
                        text.clone()
                    }
                }
                crate::flows::session::SessionTransport::MdflowTurns => {
                    crate::flows::session::build_turn_task(&meta.turns, &text)
                }
            };
            (meta.transport, prompt)
        };
        let turn_index = self.conversations.flow_sessions[index].0.turns.len();
        let message_id = format!("flow-{session_id}-retry-{turn_index}");
        let entity = self.conversations.flow_sessions[index].1.clone();
        entity.update(cx, |chat, cx| {
            chat.start_streaming(
                message_id.clone(),
                crate::protocol::ChatMessagePosition::Left,
                cx,
            );
        });
        let meta = &mut self.conversations.flow_sessions[index].0;
        // Turn submit is semantic activity (Oracle step 5): recency ordering.
        meta.touch_now();
        meta.active_turn = Some(crate::flows::session::ActiveTurn {
            run_id: None,
            message_id,
            assistant_acc: String::new(),
            current_item_id: None,
            item_acc: String::new(),
            user_text: text,
        });
        meta.state = crate::flows::session::SessionState::Working;
        if crate::runtime_policy::is_owned_evaluation() {
            cx.notify();
            return;
        }
        match transport {
            crate::flows::session::SessionTransport::CodexThread => {
                let meta = &self.conversations.flow_sessions[index].0;
                crate::flows::codex_client::codex_app_server().converse(
                    session_id,
                    &meta.cwd,
                    thread_profile.take(),
                    prompt,
                );
            }
            crate::flows::session::SessionTransport::MdflowTurns => {
                let run_id = {
                    let meta = &self.conversations.flow_sessions[index].0;
                    crate::flows::runner::launch_flow(
                        &meta.flow_id,
                        &meta.flow_name,
                        &meta.flow_path,
                        &meta.cwd,
                        crate::flows::model::FlowUxVariant::Flash,
                        crate::flows::model::EngagementMode::Background,
                        vec![("task".to_string(), prompt)],
                        std::time::Instant::now(),
                        true,
                    )
                };
                if let Some(active) = self.conversations.flow_sessions[index]
                    .0
                    .active_turn
                    .as_mut()
                {
                    active.run_id = Some(run_id);
                }
            }
        }
        self.start_flow_ux_tick(cx);
        cx.notify();
    }

    /// `flowUx` automation snapshot for getState (protocol §6).
    pub(crate) fn flow_ux_automation_snapshot(&self, cx: &gpui::App) -> serde_json::Value {
        let (desk_active, selected_flow_id) = match &self.current_view {
            AppView::FlowUxView {
                filter,
                selected_index,
                ..
            } => {
                let rows = self.flow_desk_rows(filter);
                let selected = rows.get(*selected_index).and_then(|row| match row {
                    FlowDeskRow::Flow(flow) => Some(flow.id.clone()),
                    FlowDeskRow::Session(id) => self
                        .conversations
                        .flow_sessions
                        .iter()
                        .find(|(meta, _)| meta.id == *id)
                        .map(|(meta, _)| meta.flow_id.clone()),
                    FlowDeskRow::Run(id) => crate::flows::run_registry::flow_run_registry()
                        .get(*id)
                        .map(|run| run.flow_id.clone()),
                    FlowDeskRow::InstallMdflow => Some("builtin:install-mdflow".to_string()),
                    FlowDeskRow::UpgradeMdflow => Some("builtin:upgrade-mdflow".to_string()),
                    FlowDeskRow::RetryRoster => Some("builtin:retry-flow-roster".to_string()),
                    FlowDeskRow::ClearQuery => Some("builtin:clear-flow-search".to_string()),
                    FlowDeskRow::InitFlows => Some("builtin:init-flows".to_string()),
                    FlowDeskRow::CreateFlow => Some("builtin:create-flow".to_string()),
                });
                (true, selected)
            }
            AppView::FlowSessionView { session_id } => (
                false,
                self.conversations
                    .flow_sessions
                    .iter()
                    .find(|(meta, _)| meta.id == *session_id)
                    .map(|(meta, _)| meta.flow_id.clone()),
            ),
            _ => (false, None),
        };
        let cwd = self.flow_ux_cwd();
        let safe_cwd = crate::flows::session::safe_cwd_display(&cwd);
        let roster_entry = crate::flows::catalog::flow_catalog().roster_for(&cwd);
        let sessions: Vec<crate::flows::automation::SessionSnapshot> = self
            .conversations
            .flow_sessions
            .iter()
            .map(|(meta, _)| {
                let identity = crate::flows::session::FlowSessionIdentitySnapshot::from_meta(meta);
                crate::flows::automation::SessionSnapshot {
                    id: meta.id,
                    flow_id: meta.flow_id.clone(),
                    flow_name: meta.flow_name.clone(),
                    state: meta.state.label(),
                    live: meta.state.is_live(),
                    elapsed_ms: meta.started_at.elapsed().as_millis() as u64,
                    turns: meta.turns.len(),
                    turn_in_flight: meta.active_turn.is_some(),
                    transport: match meta.transport {
                        crate::flows::session::SessionTransport::CodexThread => "codexThread",
                        crate::flows::session::SessionTransport::MdflowTurns => "mdflowTurns",
                    },
                    engine: identity.engine,
                    model: identity.model,
                    model_source: match identity.model_source {
                        crate::flows::session::FlowModelSource::Definition => "definition",
                        crate::flows::session::FlowModelSource::Runtime => "runtime",
                        crate::flows::session::FlowModelSource::Unavailable => "unavailable",
                    },
                    friendly_name: identity.friendly_name,
                    origin: identity.origin_label,
                    cwd_display: identity.cwd_display,
                    cwd_fingerprint: identity.cwd_fingerprint,
                    selection: identity.selection,
                    read_only: identity.read_only,
                    active_thread_fingerprint: identity.active_thread_fingerprint,
                    selected_thread_fingerprint: identity.selected_thread_fingerprint,
                    parent_thread_fingerprint: identity.parent_thread_fingerprint,
                    parent_retained: identity.parent_retained,
                    inherited_turn_count: identity.inherited_turn_count,
                    active_turn_count: identity.active_turn_count,
                    selected_turn_count: identity.selected_turn_count,
                    archive_count: identity.archive_count,
                    thread_count: identity.thread_count,
                    total_turn_count: identity.total_turn_count,
                    needs_rethread: identity.needs_rethread,
                    thread_ready: identity.thread_ready,
                    runtime_generation: identity.runtime_generation,
                    draft_chars: identity.draft_chars,
                    draft_fingerprint: identity.draft_fingerprint,
                    draft_generation: identity.draft_generation,
                    persistence_revision: identity.persistence_revision,
                    reliability_phase: crate::ai::reliability::phase_name(
                        &meta.reliability.state().phase,
                    )
                    .to_string(),
                    failure_code: match &meta.reliability.state().phase {
                        sk_protocol::ai_reliability::AiPhase::AwaitingRecovery {
                            failure, ..
                        } => Some(format!("{:?}", failure.code)),
                        _ => None,
                    },
                    last_failure_summary: meta
                        .turns
                        .iter()
                        .rev()
                        .find_map(|turn| turn.failure.as_ref())
                        .map(|failure| failure.safe_summary.clone()),
                }
            })
            .collect();
        let mut snapshot = crate::flows::automation::flow_ux_state(
            crate::flows::automation::FlowUxSnapshotInputs {
                active_variant: desk_active.then_some(crate::flows::model::FlowUxVariant::Flash),
                selected_flow_id: selected_flow_id.as_deref(),
                roster: Some((&roster_entry, safe_cwd.as_str())),
                preview: None,
                manager_visible: false,
                manager_focused_run_id: None,
                sessions,
            },
        );
        let active_transcript = match &self.current_view {
            AppView::FlowSessionView { session_id } => self
                .conversations
                .flow_sessions
                .iter()
                .find(|(meta, _)| meta.id == *session_id)
                .map(|(_, entity)| entity.read(cx).transcript_geometry_snapshot()),
            _ => None,
        };
        snapshot["activeTranscript"] = active_transcript.unwrap_or(serde_json::Value::Null);

        let desk_filter = match &self.current_view {
            AppView::FlowUxView { filter, .. } => filter.as_str(),
            _ => "",
        };
        let desk_state = self.flow_desk_state(desk_filter);
        snapshot["deskState"] =
            serde_json::Value::String(desk_state.automation_label().to_string());
        snapshot["deskFailure"] = match &desk_state {
            FlowDeskState::RosterFailed { failure } => serde_json::json!({
                "code": format!("{:?}", failure.failure.code),
                "diagnosticFingerprint": failure
                    .failure
                    .diagnostic
                    .as_ref()
                    .map(|diagnostic| diagnostic.fingerprint.0.clone()),
            }),
            _ => serde_json::Value::Null,
        };
        snapshot["selectedRow"] = match &self.current_view {
            AppView::FlowUxView {
                filter,
                selected_index,
                ..
            } => self
                .flow_desk_rows(filter)
                .get(*selected_index)
                .map(|row| self.flow_desk_row_descriptor(row))
                .map(|descriptor| {
                    serde_json::json!({
                        "id": descriptor.semantic_id,
                        "title": descriptor.title,
                        "detail": descriptor.detail,
                        "primaryVerb": descriptor.primary.label(),
                        "secondaryVerb": descriptor.secondary.map(FlowDeskRowVerb::label),
                        "actionsAvailable": descriptor.actions_available,
                    })
                })
                .unwrap_or(serde_json::Value::Null),
            _ => serde_json::Value::Null,
        };
        snapshot
    }
}

#[cfg(test)]
include!("flow_ux_tests.rs");
