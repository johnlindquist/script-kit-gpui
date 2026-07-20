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
    Flow(crate::flows::model::FlowDescriptor),
    /// mdflow missing: the actionable install affordance (Enter runs the
    /// install in the Quick Terminal).
    InstallMdflow,
    /// mdflow present but the roster is empty: offer the `md init` starter
    /// scaffold.
    InitFlows,
    /// The plain-English creation affordance (always last).
    CreateFlow,
}

/// How a settled turn presents in the transcript: normal completion, a quiet
/// user-initiated stop (never the red error treatment), or a real failure.
#[derive(Clone)]
pub(crate) enum FlowTurnOutcome {
    Ok,
    Stopped,
    Failed(String),
}

/// What the Flow Desk ⌘K dialog acts on. Derived fresh from view state at
/// toggle/execute time so the popup never captures a stale row.
#[derive(Clone)]
pub(crate) enum FlowDeskSubject {
    /// A flow identity row (or the desk's flow list generally).
    Flow(crate::flows::model::FlowDescriptor),
    /// A conversation session by id — selected row or the open session view.
    Session(u64),
    /// A background registry run by local id.
    Run(u64),
    /// The Create Flow affordance.
    Create,
}

/// Escape-origin capture for a flow session view, decided at open time from
/// the view being left. A session escapes back to the surface the user
/// actually came from:
///
/// - Conversation Desk (`FlowUxView`) → back to the desk.
/// - Session→session switches keep the origin already captured.
/// - Everything else (main launcher rows, actions payloads, protocol opens)
///   → `go_back_or_close`, i.e. the main menu or window-hide.
///
/// Routing a main-menu-launched session through the desk inserts a surface
/// the user never visited, which reads as a swallowed Escape on the way
/// out. That is an escape-ladder violation; keep this function the single
/// decision point.
pub(crate) fn flow_session_returns_to_desk(
    view_being_left: &AppView,
    previously_captured: bool,
) -> bool {
    match view_being_left {
        AppView::FlowUxView { .. } => true,
        AppView::FlowSessionView { .. } => previously_captured,
        _ => false,
    }
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

fn remove_flow_session<T>(
    sessions: &mut Vec<(crate::flows::session::FlowSessionMeta, T)>,
    session_id: u64,
) -> Option<(crate::flows::session::FlowSessionMeta, T)> {
    let index = sessions
        .iter()
        .position(|(meta, _)| meta.id == session_id)?;
    Some(sessions.remove(index))
}

impl ScriptListApp {
    /// Effective cwd for flow discovery: the spine cwd chip when set,
    /// otherwise $HOME. mdflow resolves project vs global flows from here.
    pub(crate) fn flow_ux_cwd(&self) -> String {
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
        if self.flow_ux_tick_running {
            return;
        }
        self.flow_ux_tick_running = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(120))
                    .await;
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

    /// Build the selectable desk rows for a filter: Active/recent sessions
    /// first (newest first), then background runs (active before finished —
    /// run-once and workflows are supervised HERE, never invisible), then
    /// matching flows, then onboarding affordances, then Create Flow.
    pub(crate) fn flow_desk_rows(&self, filter: &str) -> Vec<FlowDeskRow> {
        let mut rows: Vec<FlowDeskRow> = Vec::new();
        let query = filter.trim().to_lowercase();

        let mut sessions: Vec<&crate::flows::session::FlowSessionMeta> =
            self.flow_sessions.iter().map(|(meta, _)| meta).collect();
        sessions.sort_by(|a, b| b.id.cmp(&a.id));
        for meta in sessions {
            let matches = query.is_empty()
                || meta.friendly_name.to_lowercase().contains(&query)
                || meta.flow_name.to_lowercase().contains(&query);
            if matches {
                rows.push(FlowDeskRow::Session(meta.id));
            }
        }

        // Bare registry runs (conversation-turn runs settle inside their
        // session transcript and never appear as rows). Active first, then
        // finished, newest first within each group.
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
        for run in runs {
            rows.push(FlowDeskRow::Run(run.local_id));
        }

        let corpus = self.flow_desk_corpus();
        for flow in crate::flows::catalog::filter_flows(&corpus, filter) {
            rows.push(FlowDeskRow::Flow(flow.clone()));
        }

        // Onboarding affordances only on the unfiltered desk: a search query
        // must never grow setup rows.
        if query.is_empty() {
            if crate::flows::catalog::mdflow_binary().is_none() {
                rows.push(FlowDeskRow::InstallMdflow);
            } else {
                let cwd = self.flow_ux_cwd();
                let roster = crate::flows::catalog::flow_catalog().roster_for(&cwd);
                let ready_and_empty =
                    matches!(roster.status, crate::flows::catalog::RosterStatus::Ready)
                        && roster.flows.is_empty();
                if ready_and_empty {
                    rows.push(FlowDeskRow::InitFlows);
                }
            }
        }

        rows.push(FlowDeskRow::CreateFlow);
        rows
    }

    fn flow_session_index(&self, session_id: u64) -> Option<usize> {
        self.flow_sessions
            .iter()
            .position(|(meta, _)| meta.id == session_id)
    }

    /// Activate the desk's selected row — Enter (`run_once: false`) and ⇧↵
    /// (`run_once: true`) share this with the native footer buttons so
    /// keyboard and footer can never diverge.
    pub(crate) fn flow_desk_activate_selected(
        &mut self,
        run_once: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let AppView::FlowUxView {
            filter,
            selected_index,
            ..
        } = &self.current_view
        else {
            return;
        };
        let filter = filter.clone();
        let selected = *selected_index;
        let rows = self.flow_desk_rows(&filter);
        let Some(row) = rows.get(selected).cloned() else {
            return;
        };
        match row {
            FlowDeskRow::Session(session_id) => {
                self.open_flow_session(session_id, cx);
            }
            FlowDeskRow::Run(_) => {
                // A run row's Enter opens its supervision actions (cancel,
                // copy output, clear finished) — the same menu ⌘K shows.
                self.toggle_flow_desk_actions(window, cx);
            }
            FlowDeskRow::Flow(flow) => {
                if flow.interactive {
                    // The frozen contract: `--events` implies non-interactive
                    // and a TTY-only flow must get a REAL terminal, never a
                    // faked chat (protocol §3 "Open in Terminal").
                    self.open_flow_in_terminal(&flow, cx);
                } else if run_once || flow.is_workflow {
                    // Workflows (DAGs) are run-once by nature; ⇧↵ is
                    // explicit run-once for anything.
                    self.flow_desk_run_once(&flow, cx);
                } else {
                    // Typed text rides along as the first message only when
                    // it reads like a request (multi-word, not the flow's
                    // own name) — fuzzy lookups must never become the task.
                    let trimmed = filter.trim();
                    let lowered = trimmed.to_lowercase();
                    let is_name_lookup = trimmed.is_empty()
                        || !trimmed.contains(' ')
                        || flow.name.to_lowercase() == lowered
                        || flow.friendly_name().to_lowercase() == lowered;
                    let first_message = (!is_name_lookup).then(|| trimmed.to_string());
                    self.resume_or_start_flow_session(&flow, first_message, cx);
                }
            }
            FlowDeskRow::InstallMdflow => {
                self.open_quick_terminal_with_command(None, "npm i -g mdflow".to_string(), cx);
            }
            FlowDeskRow::InitFlows => {
                let cwd = self.flow_ux_cwd();
                self.open_quick_terminal_with_command(
                    Some(std::path::PathBuf::from(cwd)),
                    "md init".to_string(),
                    cx,
                );
            }
            FlowDeskRow::CreateFlow => {
                self.start_flow_create_session(cx);
            }
        }
        cx.notify();
    }

    /// Mouse contract for desk rows (matching the main list's conventions):
    /// clicking an unselected row selects it; clicking the selected row
    /// activates it with Enter semantics.
    pub(crate) fn flow_desk_click_row(
        &mut self,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let selected = match &self.current_view {
            AppView::FlowUxView { selected_index, .. } => *selected_index,
            _ => return,
        };
        if selected == ix {
            self.flow_desk_activate_selected(false, window, cx);
        } else if let AppView::FlowUxView { selected_index, .. } = &mut self.current_view {
            *selected_index = ix;
            self.flow_ux_scroll_handle
                .scroll_to_item(ix, ScrollStrategy::Nearest);
            cx.notify();
        }
    }

    /// Honest transport for TTY-only flows: run in the shared Quick Terminal
    /// (wrapper command when one exists, else `md <path>`).
    fn open_flow_in_terminal(
        &mut self,
        flow: &crate::flows::model::FlowDescriptor,
        cx: &mut Context<Self>,
    ) {
        let cwd = self.flow_ux_cwd();
        let command = flow
            .wrapper_command
            .clone()
            .unwrap_or_else(|| format!("md {}", shell_escape_path(&flow.path)));
        tracing::info!(
            target: "script_kit::flows",
            event = "flow_open_in_terminal",
            flow_id = %flow.id,
            "Interactive flow — opening in Quick Terminal"
        );
        self.open_quick_terminal_with_command(Some(std::path::PathBuf::from(cwd)), command, cx);
    }

    // ------------------------------------------------------------------
    // Conversation lifecycle
    // ------------------------------------------------------------------

    /// Enter-on-a-flow contract: Enter means "converse with this flow" —
    /// resume the conversation the user already has and only start a blank
    /// Threadline when there is nothing to resume. Order: live in-memory
    /// session first, then the persisted transcript from a previous app run
    /// (2026-07-10: a dev restart stranded an active GOG Gmail conversation
    /// and every launcher Enter landed in a blank composer).
    pub(crate) fn resume_or_start_flow_session(
        &mut self,
        flow: &crate::flows::model::FlowDescriptor,
        initial_message: Option<String>,
        cx: &mut Context<Self>,
    ) {
        // Identity is flow id + definition path: `project:review` exists in
        // many projects, and matching by id alone reattached (or restored)
        // the WRONG project's conversation (2026-07-11 audit P0).
        if let Some(index) = self.flow_sessions.iter().rposition(|(meta, _)| {
            meta.flow_id == flow.id && meta.flow_path == flow.path && meta.state.is_live()
        }) {
            let (session_id, went_stale) = {
                let meta = &mut self.flow_sessions[index].0;
                let current_mtime = crate::flows::session::flow_definition_mtime_ms(&flow.path);
                let went_stale = current_mtime != meta.flow_mtime_ms;
                if went_stale {
                    // The definition changed since the engine contract was
                    // resolved: drop the protocol thread so the next submit
                    // re-threads with the fresh contract + transcript rollup
                    // (same recovery path as engine death).
                    meta.needs_rethread = true;
                    meta.thread_ready = !matches!(
                        meta.transport,
                        crate::flows::session::SessionTransport::CodexThread
                    );
                    meta.flow_mtime_ms = current_mtime;
                }
                (meta.id, went_stale)
            };
            if went_stale
                && matches!(
                    self.flow_sessions[index].0.transport,
                    crate::flows::session::SessionTransport::CodexThread
                )
            {
                crate::flows::codex_client::codex_app_server().forget_session(session_id);
            }
            tracing::info!(
                target: "script_kit::flows",
                event = "flow_session_reattach",
                session_id,
                flow_id = %flow.id,
                went_stale,
                "Reattaching to the live flow conversation"
            );
            self.open_flow_session(session_id, cx);
            if let Some(message) = initial_message {
                self.submit_flow_chat_message(session_id, message, cx);
            }
            return;
        }
        if let Some(snapshot) =
            crate::flows::session::load_persisted_conversation(&flow.id, &flow.path)
        {
            self.restore_flow_session(flow, snapshot, initial_message, cx);
            return;
        }
        self.start_flow_session(flow, initial_message, cx);
    }

    /// Rebuild a conversation persisted by a previous app run: replay the
    /// transcript into a fresh Threadline and mark the session for a
    /// re-thread so the next submit carries the rolled-up history.
    fn restore_flow_session(
        &mut self,
        flow: &crate::flows::model::FlowDescriptor,
        snapshot: crate::flows::session::PersistedFlowConversation,
        initial_message: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let session_id = self.create_flow_session(flow, cx);
        let Some(index) = self.flow_session_index(session_id) else {
            return;
        };
        let entity = self.flow_sessions[index].1.clone();
        entity.update(cx, |chat, cx| {
            for turn in &snapshot.turns {
                chat.add_message(
                    crate::protocol::ChatPromptMessage::user(turn.user.clone()),
                    cx,
                );
                if !turn.assistant.is_empty() {
                    chat.add_message(
                        crate::protocol::ChatPromptMessage::assistant(turn.assistant.clone()),
                        cx,
                    );
                }
            }
        });
        let meta = &mut self.flow_sessions[index].0;
        meta.turns = snapshot
            .turns
            .iter()
            .map(|turn| crate::flows::session::SessionTurn {
                user: turn.user.clone(),
                assistant: turn.assistant.clone(),
            })
            .collect();
        meta.needs_rethread = true;
        tracing::info!(
            target: "script_kit::flows",
            event = "flow_session_restored",
            session_id,
            flow_id = %flow.id,
            turns = meta.turns.len(),
            "Restored persisted flow conversation"
        );
        self.open_flow_session(session_id, cx);
        self.start_flow_ux_tick(cx);
        if let Some(message) = initial_message {
            self.submit_flow_chat_message(session_id, message, cx);
        }
    }

    /// Start a conversation with a flow: create its Threadline (ChatPrompt)
    /// session and show it. `initial_message` (Tab router / typed text)
    /// becomes the first submitted turn.
    pub(crate) fn start_flow_session(
        &mut self,
        flow: &crate::flows::model::FlowDescriptor,
        initial_message: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let session_id = self.create_flow_session(flow, cx);
        self.open_flow_session(session_id, cx);
        self.start_flow_ux_tick(cx);
        if let Some(message) = initial_message {
            self.submit_flow_chat_message(session_id, message, cx);
        }
    }

    /// Create the session (Threadline entity + meta + engine warm-up)
    /// without opening it — shared by fresh starts and restores.
    fn create_flow_session(
        &mut self,
        flow: &crate::flows::model::FlowDescriptor,
        cx: &mut Context<Self>,
    ) -> u64 {
        let cwd = self.flow_ux_cwd();
        crate::flows::manager_window::remember_flow_cwd(&cwd);
        self.flow_session_counter += 1;
        let session_id = self.flow_session_counter;
        let transport = crate::flows::session::SessionTransport::for_engine(&flow.engine);

        tracing::info!(
            target: "script_kit::flows",
            event = "flow_session_start",
            session_id,
            flow_id = %flow.id,
            transport = ?transport,
            "Starting flow conversation"
        );

        let friendly = flow.friendly_name();
        let submit_sender = self.flow_chat_sender.clone();
        let submit_callback: crate::prompts::ChatSubmitCallback =
            std::sync::Arc::new(move |_id: String, text: String| {
                let _ = submit_sender
                    .try_send(crate::flows::session::FlowChatRequest::Submit { session_id, text });
            });
        let escape_sender = self.flow_chat_sender.clone();
        let escape_callback: crate::prompts::ChatEscapeCallback =
            std::sync::Arc::new(move |_id: String| {
                let _ = escape_sender
                    .try_send(crate::flows::session::FlowChatRequest::Background { session_id });
            });

        let mut chat = crate::prompts::ChatPrompt::new(
            format!("flow-session-{session_id}"),
            Some(format!("Message {friendly}…")),
            vec![],
            None,
            None,
            self.focus_handle.clone(),
            submit_callback,
            std::sync::Arc::clone(&self.theme),
        )
        .with_title(friendly.clone())
        .with_save_history(false)
        .with_escape_callback(escape_callback)
        .with_escape_over_stop(true)
        .with_external_footer(true)
        .with_external_header(true)
        .with_external_input(true)
        .with_empty_state_note(
            flow.description
                .clone()
                .unwrap_or_else(|| format!("Converse with {friendly}.")),
        )
        .with_top_aligned_turns();
        let actions_sender = self.flow_chat_sender.clone();
        chat.set_on_show_actions(std::sync::Arc::new(move |_id: String| {
            let _ = actions_sender
                .try_send(crate::flows::session::FlowChatRequest::ShowActions { session_id });
        }));
        let entity = cx.new(|_| chat);

        let meta = crate::flows::session::FlowSessionMeta {
            id: session_id,
            flow_id: flow.id.clone(),
            flow_name: flow.name.clone(),
            friendly_name: friendly,
            origin: flow.origin_label().to_string(),
            engine: flow.engine.clone(),
            flow_path: flow.path.clone(),
            flow_mtime_ms: crate::flows::session::flow_definition_mtime_ms(&flow.path),
            cwd,
            transport,
            state: crate::flows::session::SessionState::NeedsYou,
            started_at: std::time::Instant::now(),
            turns: Vec::new(),
            active_turn: None,
            thread_ready: !matches!(
                transport,
                crate::flows::session::SessionTransport::CodexThread
            ),
            needs_rethread: false,
        };
        // Codex transport: warm the protocol thread while the user types
        // their first message. File read + server spawn + thread/start all
        // happen off the GPUI thread; failures surface as SessionFailed
        // through the normal event drain.
        if matches!(
            meta.transport,
            crate::flows::session::SessionTransport::CodexThread
        ) {
            let flow_path = meta.flow_path.clone();
            let warm_cwd = meta.cwd.clone();
            std::thread::Builder::new()
                .name("flow-thread-warm".into())
                .spawn(move || {
                    let profile = std::fs::read_to_string(&flow_path)
                        .map(|markdown| {
                            crate::flows::session::resolve_flow_thread_contract(&markdown, "")
                                .profile
                        })
                        .unwrap_or_default();
                    crate::flows::codex_client::codex_app_server()
                        .prepare_thread(session_id, &warm_cwd, profile);
                })
                .ok();
        }
        self.flow_sessions.push((meta, entity));
        session_id
    }

    /// Submit one user message on a session: echo it into the transcript,
    /// open a streaming assistant bubble, and dispatch the turn on the
    /// session's transport. One turn in flight per session.
    pub(crate) fn submit_flow_chat_message(
        &mut self,
        session_id: u64,
        text: String,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.flow_session_index(session_id) else {
            return;
        };
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }
        if self.flow_sessions[index].0.active_turn.is_some() {
            self.toast_manager.push(
                crate::components::toast::Toast::error(
                    "Still working — stop the current turn first (⌘K)".to_string(),
                    &self.theme,
                )
                .duration_ms(Some(2500)),
            );
            cx.notify();
            return;
        }

        let mut thread_profile: Option<crate::flows::session::FlowThreadProfile> = None;
        let mut flow_unreadable: Option<String> = None;
        let (transport, prompt) = {
            let meta = &self.flow_sessions[index].0;
            let prompt = match meta.transport {
                crate::flows::session::SessionTransport::CodexThread => {
                    if meta.turns.is_empty() || meta.needs_rethread {
                        // First turn — or the first turn on a FRESH thread
                        // after the engine died — resolves the flow's
                        // contract: mission + any pinned model/sandbox go to
                        // thread/start. A re-thread carries the transcript
                        // rollup as its task so the conversation survives.
                        // An unreadable definition fails CLOSED — never
                        // degrade into a generic codex chat wearing the
                        // flow's name.
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
                            Err(err) => {
                                flow_unreadable = Some(format!(
                                    "Flow definition unreadable: {} ({err})",
                                    meta.flow_path
                                ));
                                String::new()
                            }
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

        let turn_index = self.flow_sessions[index].0.turns.len();
        let message_id = format!("flow-{session_id}-turn-{turn_index}");
        let entity = self.flow_sessions[index].1.clone();
        let user_text = text.clone();
        entity.update(cx, |chat, cx| {
            chat.add_message(crate::protocol::ChatPromptMessage::user(user_text), cx);
            chat.start_streaming(
                message_id.clone(),
                crate::protocol::ChatMessagePosition::Left,
                cx,
            );
        });

        if let Some(error) = flow_unreadable {
            tracing::warn!(
                target: "script_kit::flows",
                event = "flow_turn_failed_closed",
                session_id,
                error = %error,
                "Flow definition unreadable — failing the turn closed"
            );
            let meta = &mut self.flow_sessions[index].0;
            meta.active_turn = Some(crate::flows::session::ActiveTurn {
                run_id: None,
                message_id,
                assistant_acc: String::new(),
                current_item_id: None,
                item_acc: String::new(),
                user_text: text,
            });
            self.finish_flow_turn(
                session_id,
                crate::flows::session::SessionState::Done(None),
                FlowTurnOutcome::Failed(error),
                cx,
            );
            cx.notify();
            return;
        }

        tracing::info!(
            target: "script_kit::flows",
            event = "flow_turn_submit",
            session_id,
            transport = ?transport,
            prompt_len = prompt.len(),
            "Submitting flow turn"
        );

        let run_id = match transport {
            crate::flows::session::SessionTransport::CodexThread => {
                let meta = &self.flow_sessions[index].0;
                crate::flows::codex_client::codex_app_server().converse(
                    session_id,
                    &meta.cwd,
                    thread_profile.take(),
                    prompt,
                );
                None
            }
            crate::flows::session::SessionTransport::MdflowTurns => {
                let meta = &self.flow_sessions[index].0;
                Some(crate::flows::runner::launch_flow(
                    &meta.flow_id,
                    &meta.flow_name,
                    &meta.flow_path,
                    &meta.cwd,
                    crate::flows::model::FlowUxVariant::Flash,
                    crate::flows::model::EngagementMode::Background,
                    vec![("task".to_string(), prompt)],
                    std::time::Instant::now(),
                    // Conversation turn: stream from the append-only capture,
                    // never the bounded display tail (cursor corruption P0).
                    true,
                ))
            }
        };

        let meta = &mut self.flow_sessions[index].0;
        meta.active_turn = Some(crate::flows::session::ActiveTurn {
            run_id,
            message_id,
            assistant_acc: String::new(),
            current_item_id: None,
            item_acc: String::new(),
            user_text: text,
        });
        meta.needs_rethread = false;
        meta.state = crate::flows::session::SessionState::Working;
        self.start_flow_ux_tick(cx);
        cx.notify();
    }

    /// Append streamed assistant text to a session's open turn.
    fn append_flow_turn_text(&mut self, session_id: u64, delta: &str, cx: &mut Context<Self>) {
        let Some(index) = self.flow_session_index(session_id) else {
            return;
        };
        let entity = self.flow_sessions[index].1.clone();
        let Some(active) = self.flow_sessions[index].0.active_turn.as_mut() else {
            return;
        };
        active.assistant_acc.push_str(delta);
        active.item_acc.push_str(delta);
        let message_id = active.message_id.clone();
        let delta = delta.to_string();
        entity.update(cx, |chat, cx| {
            chat.append_chunk(&message_id, &delta, cx);
        });
    }

    /// Enter an agentMessage item: when the turn moves to a NEW item after
    /// prior text, insert a paragraph break so consecutive items never
    /// butt-join ("…summarizing.The listed…"), then reset the per-item
    /// accumulator that `item/completed` reconciliation compares against.
    fn begin_flow_turn_item(&mut self, session_id: u64, item_id: &str, cx: &mut Context<Self>) {
        let Some(index) = self.flow_session_index(session_id) else {
            return;
        };
        let needs_break = {
            let Some(active) = self.flow_sessions[index].0.active_turn.as_mut() else {
                return;
            };
            active.enter_item(item_id)
        };
        if needs_break {
            self.append_flow_turn_text(session_id, "\n\n", cx);
            // The break belongs to the boundary, not the new item's text.
            if let Some(active) = self.flow_sessions[index].0.active_turn.as_mut() {
                active.item_acc.clear();
            }
        }
    }

    /// Settle a session's open turn: close the streaming bubble, surface the
    /// outcome, commit the SessionTurn, set state. A user-initiated stop is
    /// NOT an error — it renders as a quiet italic caption, never the red
    /// error treatment.
    fn finish_flow_turn(
        &mut self,
        session_id: u64,
        state: crate::flows::session::SessionState,
        outcome: FlowTurnOutcome,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.flow_session_index(session_id) else {
            return;
        };
        let Some(active) = self.flow_sessions[index].0.active_turn.take() else {
            return;
        };
        let entity = self.flow_sessions[index].1.clone();
        let message_id = active.message_id.clone();
        let had_error = matches!(outcome, FlowTurnOutcome::Failed(_));
        let stopped_caption = match &outcome {
            FlowTurnOutcome::Stopped if active.assistant_acc.is_empty() => {
                Some("*Stopped.*".to_string())
            }
            FlowTurnOutcome::Stopped => Some("\n\n*Stopped.*".to_string()),
            _ => None,
        };
        entity.update(cx, |chat, cx| {
            // append_chunk is gated on the live stream — captions go in
            // before the stream closes.
            if let Some(caption) = stopped_caption {
                chat.append_chunk(&message_id, &caption, cx);
            }
            chat.complete_streaming(&message_id, cx);
            if let FlowTurnOutcome::Failed(note) = outcome {
                chat.set_message_error(&message_id, note, cx);
            }
        });
        let meta = &mut self.flow_sessions[index].0;
        meta.turns.push(crate::flows::session::SessionTurn {
            user: active.user_text,
            assistant: active.assistant_acc,
        });
        meta.state = state;
        // Snapshot the conversation off-thread so an app restart can restore
        // it (resume_or_start_flow_session); in-memory sessions die with the
        // process.
        let flow_id = meta.flow_id.clone();
        let flow_path = meta.flow_path.clone();
        let turns = meta.turns.clone();
        std::thread::Builder::new()
            .name("flow-conversation-persist".into())
            .spawn(move || {
                crate::flows::session::persist_conversation(&flow_id, &flow_path, &turns)
            })
            .ok();
        tracing::info!(
            target: "script_kit::flows",
            event = "flow_turn_settled",
            session_id,
            state = state.label(),
            had_error,
            "Flow turn settled"
        );
    }

    /// Apply one codex app-server event to its session.
    fn apply_flow_thread_event(
        &mut self,
        event: crate::flows::codex_client::FlowThreadEvent,
        cx: &mut Context<Self>,
    ) {
        use crate::flows::codex_client::FlowThreadEvent;
        use crate::flows::session::SessionState;
        match event {
            FlowThreadEvent::ThreadStarted { session_id, model } => {
                if let Some(index) = self.flow_session_index(session_id) {
                    self.flow_sessions[index].0.thread_ready = true;
                    if !model.is_empty() {
                        self.flow_sessions[index].0.engine = format!("codex · {model}");
                    }
                }
            }
            FlowThreadEvent::TurnStarted { session_id } => {
                if let Some(index) = self.flow_session_index(session_id) {
                    if self.flow_sessions[index].0.active_turn.is_some() {
                        self.flow_sessions[index].0.state = SessionState::Working;
                    }
                }
            }
            FlowThreadEvent::AgentDelta {
                session_id,
                item_id,
                delta,
            } => {
                self.begin_flow_turn_item(session_id, &item_id, cx);
                self.append_flow_turn_text(session_id, &delta, cx);
            }
            FlowThreadEvent::AgentMessageFinal {
                session_id,
                item_id,
                text,
            } => {
                // Authoritative full text of ONE item: append whatever its
                // deltas missed (deltas can lag or be skipped entirely).
                // Reconcile against the item accumulator, never the whole
                // turn — a turn carries several items and comparing across
                // items would drop or butt-join them.
                self.begin_flow_turn_item(session_id, &item_id, cx);
                let Some(index) = self.flow_session_index(session_id) else {
                    return;
                };
                let item_acc = self.flow_sessions[index]
                    .0
                    .active_turn
                    .as_ref()
                    .map(|active| active.item_acc.clone())
                    .unwrap_or_default();
                if text.len() > item_acc.len() && text.starts_with(&item_acc) {
                    let suffix = text[item_acc.len()..].to_string();
                    self.append_flow_turn_text(session_id, &suffix, cx);
                } else if item_acc.is_empty() && !text.is_empty() {
                    self.append_flow_turn_text(session_id, &text, cx);
                }
            }
            FlowThreadEvent::TurnCompleted {
                session_id,
                status,
                error,
            } => {
                let (state, outcome) = match status.as_str() {
                    "completed" => (SessionState::NeedsYou, FlowTurnOutcome::Ok),
                    "interrupted" => (SessionState::NeedsYou, FlowTurnOutcome::Stopped),
                    _ => (
                        SessionState::Done(None),
                        FlowTurnOutcome::Failed(error.unwrap_or_else(|| "Turn failed".to_string())),
                    ),
                };
                self.finish_flow_turn(session_id, state, outcome, cx);
            }
            FlowThreadEvent::SessionFailed { session_id, error } => {
                let Some(index) = self.flow_session_index(session_id) else {
                    return;
                };
                // The protocol thread is gone (server death or thread/start
                // failure). The next submit must re-thread with the flow's
                // contract + transcript rollup, and the footer must show
                // Connecting again instead of pretending the thread lives.
                self.flow_sessions[index].0.thread_ready = false;
                self.flow_sessions[index].0.needs_rethread = true;
                if self.flow_sessions[index].0.active_turn.is_some() {
                    self.finish_flow_turn(
                        session_id,
                        crate::flows::session::SessionState::Done(None),
                        FlowTurnOutcome::Failed(error),
                        cx,
                    );
                } else {
                    self.flow_sessions[index].0.state =
                        crate::flows::session::SessionState::Done(None);
                }
            }
        }
    }

    /// Stream mdflow-turn run output into transcripts and settle finished
    /// turns. Returns true when anything changed.
    fn sync_mdflow_turns(&mut self, cx: &mut Context<Self>) -> bool {
        let registry = crate::flows::run_registry::flow_run_registry();
        let mut dirty = false;
        for index in 0..self.flow_sessions.len() {
            let (session_id, run_id, acc_len) = {
                let meta = &self.flow_sessions[index].0;
                let Some(active) = &meta.active_turn else {
                    continue;
                };
                let Some(run_id) = active.run_id else {
                    continue;
                };
                (meta.id, run_id, active.assistant_acc.len())
            };
            let Some(run) = registry.get(run_id) else {
                self.finish_flow_turn(
                    session_id,
                    crate::flows::session::SessionState::Done(None),
                    FlowTurnOutcome::Failed("run disappeared from the registry".to_string()),
                    cx,
                );
                dirty = true;
                continue;
            };
            // Stream from the append-only conversation capture. The bounded
            // display tail front-evicts, which broke the byte cursor on long
            // turns (silent stalls / garbled text — 2026-07-11 audit P0);
            // the capture never evicts, so `acc_len` is always a valid char
            // boundary within it.
            let full = run.conversation_stdout.clone().unwrap_or_default();
            if full.len() > acc_len {
                if let Some(delta) = full.get(acc_len..) {
                    let delta = delta.to_string();
                    self.append_flow_turn_text(session_id, &delta, cx);
                    dirty = true;
                }
            } else if run.conversation_truncated && acc_len == full.len() {
                // The capture froze at its cap: say so once. Appending the
                // caption makes acc_len exceed the frozen capture length, so
                // this branch can never repeat.
                self.append_flow_turn_text(
                    session_id,
                    "\n\n*Output truncated — this turn exceeded the 4 MB capture limit.*",
                    cx,
                );
                dirty = true;
            }
            if run.phase.is_terminal() {
                use crate::flows::model::RunPhase;
                use crate::flows::session::SessionState;
                let (state, outcome) = match run.phase {
                    RunPhase::Succeeded => (SessionState::NeedsYou, FlowTurnOutcome::Ok),
                    RunPhase::Cancelled => (SessionState::NeedsYou, FlowTurnOutcome::Stopped),
                    _ => (
                        SessionState::Done(run.exit_code.map(|code| code as i32)),
                        FlowTurnOutcome::Failed(
                            run.error_message
                                .clone()
                                .unwrap_or_else(|| run.display_status()),
                        ),
                    ),
                };
                self.finish_flow_turn(session_id, state, outcome, cx);
                dirty = true;
            }
        }
        dirty
    }

    /// Show an existing session (same ChatPrompt entity — the reattach).
    pub(crate) fn open_flow_session(&mut self, session_id: u64, cx: &mut Context<Self>) {
        let Some(index) = self.flow_session_index(session_id) else {
            return;
        };
        let friendly = self.flow_sessions[index].0.friendly_name.clone();
        self.flow_session_return_to_desk =
            flow_session_returns_to_desk(&self.current_view, self.flow_session_return_to_desk);
        self.current_view = AppView::FlowSessionView { session_id };
        // The MAIN input is the composer (with all its context-attachment
        // features) — clear any desk query and retitle the placeholder.
        self.filter_text.clear();
        self.pending_filter_sync = true;
        self.pending_placeholder = Some(format!("Message {friendly}…"));
        self.focused_input = FocusedInput::MainFilter;
        self.pending_focus = Some(FocusTarget::MainFilter);
        cx.spawn(async move |_this, _cx| {
            crate::window_resize::resize_to_view_sync(
                crate::window_resize::ViewType::MainWindow,
                0,
            );
        })
        .detach();
        cx.notify();
    }

    /// Leave the session view without touching the process. The session
    /// stays in `flow_sessions` and reappears under Active.
    ///
    /// ESCAPE LADDER CONTRACT: Escape returns exactly ONE step, to the
    /// surface the session was actually entered from. Desk-entered sessions
    /// return to the desk; main-menu-launched (and every other) session
    /// routes through `go_back_or_close`, so the next Escape on an empty
    /// main menu hides the window. Detouring through a surface the user
    /// never visited reads as a swallowed Escape — locked by
    /// `flow_session_escape_origin` tests and
    /// `scripts/agentic/flow-session-escape-ladder-probe.ts`.
    pub(crate) fn background_flow_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let AppView::FlowSessionView { session_id } = self.current_view else {
            return;
        };
        tracing::info!(
            target: "script_kit::flows",
            event = "flow_session_backgrounded",
            session_id,
            return_to_desk = self.flow_session_return_to_desk,
            "Backgrounding flow session (process stays alive)"
        );
        if !self.flow_session_return_to_desk {
            // Entered from the main launcher (or directly): go back the way
            // the user came. `go_back_or_close` clears the shared input,
            // resets `opened_from_main_menu`, and restores the launcher —
            // or hides the window for direct opens.
            self.filter_text.clear();
            self.pending_filter_sync = true;
            self.go_back_or_close(window, cx);
            return;
        }
        // Clear the shared filter INPUT too (pending_filter_sync flushes the
        // widget on next render) — the desk must never show a stale query
        // over an unfiltered list — and restore the desk placeholder.
        self.filter_text.clear();
        self.pending_filter_sync = true;
        self.pending_placeholder = Some("Search flows...".to_string());
        self.current_view = AppView::FlowUxView {
            variant: crate::flows::model::FlowUxVariant::Flash,
            filter: String::new(),
            selected_index: 0,
            inline_run: None,
        };
        self.focused_input = FocusedInput::MainFilter;
        cx.notify();
    }

    /// Explicit stop (⌘K verb): cancel the in-flight turn only. The
    /// conversation survives and the composer stays usable.
    pub(crate) fn stop_flow_session(&mut self, session_id: u64, cx: &mut Context<Self>) {
        let Some(index) = self.flow_session_index(session_id) else {
            return;
        };
        let Some(active) = self.flow_sessions[index].0.active_turn.clone() else {
            return;
        };
        match self.flow_sessions[index].0.transport {
            crate::flows::session::SessionTransport::CodexThread => {
                crate::flows::codex_client::codex_app_server().interrupt(session_id);
                // turn/completed {status: interrupted} settles the turn.
            }
            crate::flows::session::SessionTransport::MdflowTurns => {
                if let Some(run_id) = active.run_id {
                    crate::flows::runner::cancel_run(run_id);
                    // The registry's Cancelled phase settles the turn.
                }
            }
        }
        cx.notify();
    }

    /// Explicitly end a conversation, even while a turn is in flight.
    pub(crate) fn terminate_flow_session(
        &mut self,
        session_id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.flow_session_index(session_id) else {
            return;
        };
        if let Some(active) = self.flow_sessions[index].0.active_turn.clone() {
            match self.flow_sessions[index].0.transport {
                crate::flows::session::SessionTransport::CodexThread => {
                    crate::flows::codex_client::codex_app_server().interrupt(session_id);
                }
                crate::flows::session::SessionTransport::MdflowTurns => {
                    if let Some(run_id) = active.run_id {
                        crate::flows::runner::cancel_run(run_id);
                    }
                }
            }
        }
        crate::flows::codex_client::codex_app_server().forget_session(session_id);
        let viewing = matches!(
            self.current_view,
            AppView::FlowSessionView { session_id: current } if current == session_id
        );
        // "Terminate Flow" promises to PERMANENTLY end the conversation —
        // erase the persisted transcript too, or the next activation would
        // silently restore it (2026-07-11 audit P0: UI-contract violation).
        if let Some(removed) = remove_flow_session(&mut self.flow_sessions, session_id) {
            let flow_id = removed.0.flow_id.clone();
            let flow_path = removed.0.flow_path.clone();
            std::thread::Builder::new()
                .name("flow-conversation-delete".into())
                .spawn(move || {
                    crate::flows::session::delete_persisted_conversation(&flow_id, &flow_path)
                })
                .ok();
        }
        if viewing {
            self.background_flow_session(window, cx);
        }
        cx.notify();
    }

    /// Run once in the background via the run registry (`--events`).
    pub(crate) fn flow_desk_run_once(
        &mut self,
        flow: &crate::flows::model::FlowDescriptor,
        cx: &mut Context<Self>,
    ) -> u64 {
        let cwd = self.flow_ux_cwd();
        crate::flows::manager_window::remember_flow_cwd(&cwd);
        let run_id = crate::flows::runner::launch_flow(
            &flow.id,
            &flow.name,
            &flow.path,
            &cwd,
            crate::flows::model::FlowUxVariant::Flash,
            crate::flows::model::EngagementMode::Background,
            Vec::new(),
            std::time::Instant::now(),
            false,
        );
        self.start_flow_ux_tick(cx);
        self.toast_manager.push(
            crate::components::toast::Toast::success(
                format!(
                    "{} running in background — watch it in the desk list",
                    flow.friendly_name()
                ),
                &self.theme,
            )
            .duration_ms(Some(1800)),
        );
        cx.notify();
        run_id
    }

    /// Start the plain-English creation path. `md create` is a genuinely
    /// interactive CLI wizard, so it runs in the shared Quick Terminal
    /// (honest transport) rather than being faked into a chat surface.
    pub(crate) fn start_flow_create_session(&mut self, cx: &mut Context<Self>) {
        // Pre-check: opening a terminal that immediately fails with
        // "command not found" is a dead end — point at the install
        // affordance instead.
        if crate::flows::catalog::mdflow_binary().is_none() {
            self.toast_manager.push(
                crate::components::toast::Toast::error(
                    "mdflow isn't installed — use the Install mdflow row first".to_string(),
                    &self.theme,
                )
                .duration_ms(Some(3000)),
            );
            cx.notify();
            return;
        }
        let cwd = self.flow_ux_cwd();
        tracing::info!(
            target: "script_kit::flows",
            event = "flow_create_open",
            "Opening md create in Quick Terminal"
        );
        self.open_quick_terminal_with_command(
            Some(std::path::PathBuf::from(cwd)),
            "md create".to_string(),
            cx,
        );
    }

    // ------------------------------------------------------------------
    // Tab flow router entry (from the main menu input)
    // ------------------------------------------------------------------

    /// Route free text typed in the main menu to a flow (Tab). Confident →
    /// start the conversation; the text rides along as the first message
    /// ONLY when it reads like a request rather than the flow's own name
    /// (a lookup query like "githu" must never become the agent's task).
    /// Otherwise → open the desk with the text as the filter so the user
    /// picks (the Create Flow row is always present for the no-match case).
    ///
    /// 2026-07-10: no longer wired to Tab — Tab-with-text is Quick AI again
    /// (`open_quick_ai_from_launcher`); flows stay reachable as main-menu rows.
    /// Kept for a future explicit flow-routing entry point.
    #[allow(dead_code)]
    pub(crate) fn route_text_to_flow(
        &mut self,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let corpus = self.flow_desk_corpus();
        let decision = crate::flows::router::route(&text, &corpus);
        match decision {
            crate::flows::router::RouteDecision::AutoStart { flow } => {
                let trimmed = text.trim();
                let lowered = trimmed.to_lowercase();
                let is_name_lookup = !trimmed.contains(' ')
                    || flow.name.to_lowercase() == lowered
                    || flow.friendly_name().to_lowercase() == lowered;
                let first_message = (!is_name_lookup).then(|| trimmed.to_string());
                tracing::info!(
                    target: "script_kit::flows",
                    event = "flow_router_auto_start",
                    flow_id = %flow.id,
                    query_len = text.len(),
                    carries_message = first_message.is_some(),
                    "Tab router: confident match, starting conversation"
                );
                self.resume_or_start_flow_session(&flow, first_message, cx);
            }
            crate::flows::router::RouteDecision::Candidates { .. }
            | crate::flows::router::RouteDecision::NoMatch => {
                tracing::info!(
                    target: "script_kit::flows",
                    event = "flow_router_candidates",
                    query_len = text.len(),
                    "Tab router: opening desk with candidates"
                );
                let cwd = self.flow_ux_cwd();
                crate::flows::catalog::flow_catalog().refresh(&cwd);
                self.open_builtin_filterable_view(
                    AppView::FlowUxView {
                        variant: crate::flows::model::FlowUxVariant::Flash,
                        filter: text.clone(),
                        selected_index: 0,
                        inline_run: None,
                    },
                    "Search flows...",
                    false,
                    cx,
                );
                // Seed the visible input with the routed text so the desk
                // filter and the header input agree (cwd-pick pattern).
                self.suppress_filter_events = true;
                self.gpui_input_state.update(cx, |state, cx| {
                    state.set_value(text.clone(), window, cx);
                    let len = text.len();
                    state.set_selection(len, len, window, cx);
                });
                self.suppress_filter_events = false;
                self.start_flow_ux_tick(cx);
            }
        }
    }

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
        let chrome = crate::theme::AppChromeColors::from_theme(&self.theme);
        let list_colors = crate::list_item::ListItemColors::from_theme(&self.theme);
        let cwd = self.flow_ux_cwd();
        let roster = crate::flows::catalog::flow_catalog().roster_for(&cwd);
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
        let empty_message = match roster.status {
            crate::flows::catalog::RosterStatus::Loading => {
                format!("Loading flows in {cwd_display}…")
            }
            crate::flows::catalog::RosterStatus::Legacy => {
                "mdflow is pre-protocol — upgrade with: npm i -g mdflow".to_string()
            }
            crate::flows::catalog::RosterStatus::Error => match roster.warnings.first() {
                Some(warning) => format!("Flow roster unavailable — {warning}"),
                None => format!("Flow roster unavailable in {cwd_display}"),
            },
            crate::flows::catalog::RosterStatus::Ready => String::new(),
        };

        let list_element: gpui::AnyElement = {
            let display_rows = rows.clone();
            let hovered = self.hovered_index;
            let session_meta: Vec<crate::flows::session::FlowSessionMeta> = self
                .flow_sessions
                .iter()
                .map(|(meta, _)| meta.clone())
                .collect();
            let run_meta: Vec<crate::flows::run_registry::RunSummary> = registry.run_summaries();
            let click_entity = cx.entity();
            uniform_list(
                "flow-desk-list",
                row_count,
                move |visible_range, _window, _cx| {
                    visible_range
                        .map(|ix| {
                            let is_selected = ix == selected_index;
                            let is_hovered = hovered == Some(ix);
                            let (title, description, icon) = match &display_rows[ix] {
                            FlowDeskRow::Session(session_id) => {
                                let meta =
                                    session_meta.iter().find(|m| m.id == *session_id);
                                match meta {
                                    Some(meta) => (
                                        meta.friendly_name.clone(),
                                        format!(
                                            "{} · {} · {} · conversation",
                                            meta.state.label(),
                                            meta.elapsed_label(),
                                            meta.engine,
                                        ),
                                        if meta.state.is_live() { "💬" } else { "◽" },
                                    ),
                                    None => (
                                        "Session".to_string(),
                                        "ended".to_string(),
                                        "◽",
                                    ),
                                }
                            }
                            FlowDeskRow::Run(run_id) => {
                                let run = run_meta.iter().find(|r| r.local_id == *run_id);
                                match run {
                                    Some(run) => (
                                        crate::flows::model::friendly_flow_name(
                                            &run.flow_name,
                                        ),
                                        format!(
                                            "{} · {} · {}",
                                            run.display_status,
                                            format_run_elapsed(run.elapsed_ms),
                                            run.last_output_line.as_deref().unwrap_or("—"),
                                        ),
                                        run_phase_icon(run.phase),
                                    ),
                                    None => ("Run".to_string(), "gone".to_string(), "◽"),
                                }
                            }
                            FlowDeskRow::Flow(flow) => {
                                let purpose = flow
                                    .description
                                    .clone()
                                    .unwrap_or_else(|| flow.name.clone());
                                (
                                    flow.friendly_name(),
                                    format!(
                                        "{purpose} · {} · {}",
                                        flow.engine,
                                        flow.origin_label()
                                    ),
                                    if flow.interactive {
                                        "🖥"
                                    } else if flow.is_workflow {
                                        "🧩"
                                    } else {
                                        "⚡"
                                    },
                                )
                            }
                            FlowDeskRow::InstallMdflow => (
                                "Install mdflow".to_string(),
                                "The flow engine isn't on PATH — run npm i -g mdflow in Terminal"
                                    .to_string(),
                                "⬇",
                            ),
                            FlowDeskRow::InitFlows => (
                                "Scaffold starter flows".to_string(),
                                "md init creates a flows/ roster here (no engine calls)"
                                    .to_string(),
                                "🌱",
                            ),
                            FlowDeskRow::CreateFlow => (
                                "Create a flow…".to_string(),
                                "Describe an agent in plain English (md create)"
                                    .to_string(),
                                "✚",
                            ),
                        };
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
                                    ListItem::new(title, list_colors)
                                        .description_opt(Some(description))
                                        .icon(icon)
                                        .selected(is_selected)
                                        .hovered(is_hovered)
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
        if !empty_message.is_empty() && roster.flows.is_empty() {
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
            .flow_sessions
            .iter()
            .filter(|(meta, _)| meta.state.is_live())
            .count();
        let active_runs = registry.active_count();
        let hints: Vec<gpui::SharedString> = vec![
            gpui::SharedString::from("↵ Converse"),
            gpui::SharedString::from("⇧↵ Run once"),
            gpui::SharedString::from("⌘K Actions"),
            gpui::SharedString::from("Esc Back"),
        ];
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
        let (meta, entity) = {
            let (meta, entity) = &self.flow_sessions[index];
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

        // Enter = send the main-input draft as the next turn; Esc =
        // background to the desk; ⌘K = session actions. Same shell-level
        // key routing the desk uses while the main input is focused.
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                let AppView::FlowSessionView { session_id } = this.current_view else {
                    return;
                };
                let key = event.keystroke.key.as_str();
                let has_cmd = event.keystroke.modifiers.platform;
                let has_shift = event.keystroke.modifiers.shift;
                // ⇧⌘⎋ terminates — exactly what the footer advertises. Plain
                // ⌘⎋ must NOT destroy a conversation (2026-07-11 audit: the
                // handler fired on ⌘⎋ while the hint said ⇧⌘⎋).
                if has_cmd && has_shift && is_key_escape(key) && !this.show_actions_popup {
                    this.terminate_flow_session(session_id, window, cx);
                    cx.stop_propagation();
                    return;
                }
                if is_key_escape(key) && !this.show_actions_popup {
                    this.background_flow_session(window, cx);
                    cx.stop_propagation();
                    return;
                }
                if has_cmd && key.eq_ignore_ascii_case("k") {
                    this.dispatch_actions_toggle_for_current_view(window, cx, "flow_session_chat");
                    cx.stop_propagation();
                    return;
                }
                if is_key_enter(key) {
                    let text = this.filter_text.trim().to_string();
                    if !text.is_empty() {
                        this.set_filter_text_immediate(String::new(), window, cx);
                        this.submit_flow_chat_message(session_id, text, cx);
                    }
                    cx.stop_propagation();
                }
            },
        );

        // Honest state rides as the footer hint strip's leading status text —
        // the same slot ChatPrompt's own footer uses ("Streaming · model").
        // No ticking elapsed timer in chrome; the desk row carries elapsed.
        let status_text = if meta.active_turn.is_some() && !meta.thread_ready {
            format!("Connecting · {}", meta.engine)
        } else if meta.active_turn.is_some() {
            format!("Working · {}", meta.engine)
        } else {
            meta.engine.clone()
        };
        let hints: Vec<gpui::SharedString> = vec![
            gpui::SharedString::from("↵ Send"),
            gpui::SharedString::from("⇧⌘⎋ Terminate Flow"),
            gpui::SharedString::from("Esc Desk"),
            gpui::SharedString::from("⌘K Actions"),
        ];
        let footer = self.main_window_footer_slot(crate::components::render_simple_hint_strip(
            hints,
            Some(crate::components::render_hint_strip_leading_text(
                status_text,
                self.theme.colors.text.primary,
            )),
        ));

        let menu_def = self.current_main_menu_theme.def();
        let shell = menu_def.shell;
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
                header: self.render_builtin_main_input_header(trailing, cx),
                divider: crate::components::main_view_chrome::MainViewDividerChrome {
                    margin_x: shell.divider_margin_x,
                    height: shell.divider_height,
                    visible: shell.divider_height > 0.0,
                },
                main: div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.))
                    .w_full()
                    .child(entity)
                    .into_any_element(),
                footer,
                overlays: Vec::new(),
            },
        )
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
                        .flow_sessions
                        .iter()
                        .find(|(meta, _)| meta.id == *id)
                        .map(|(meta, _)| meta.flow_id.clone()),
                    FlowDeskRow::Run(id) => crate::flows::run_registry::flow_run_registry()
                        .get(*id)
                        .map(|run| run.flow_id.clone()),
                    FlowDeskRow::InstallMdflow => Some("builtin:install-mdflow".to_string()),
                    FlowDeskRow::InitFlows => Some("builtin:init-flows".to_string()),
                    FlowDeskRow::CreateFlow => Some("builtin:create-flow".to_string()),
                });
                (true, selected)
            }
            AppView::FlowSessionView { session_id } => (
                false,
                self.flow_sessions
                    .iter()
                    .find(|(meta, _)| meta.id == *session_id)
                    .map(|(meta, _)| meta.flow_id.clone()),
            ),
            _ => (false, None),
        };
        let cwd = self.flow_ux_cwd();
        let roster_entry = crate::flows::catalog::flow_catalog().roster_for(&cwd);
        let sessions: Vec<crate::flows::automation::SessionSnapshot> = self
            .flow_sessions
            .iter()
            .map(|(meta, _)| crate::flows::automation::SessionSnapshot {
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
                engine: meta.engine.clone(),
            })
            .collect();
        let mut snapshot = crate::flows::automation::flow_ux_state(crate::flows::automation::FlowUxSnapshotInputs {
            active_variant: desk_active.then_some(crate::flows::model::FlowUxVariant::Flash),
            selected_flow_id: selected_flow_id.as_deref(),
            roster: Some((&roster_entry, cwd.as_str())),
            preview: None,
            manager_visible: false,
            manager_focused_run_id: None,
            sessions,
        });
        let active_transcript = match &self.current_view {
            AppView::FlowSessionView { session_id } => self
                .flow_sessions
                .iter()
                .find(|(meta, _)| meta.id == *session_id)
                .map(|(_, entity)| entity.read(cx).transcript_geometry_snapshot()),
            _ => None,
        };
        snapshot["activeTranscript"] = active_transcript.unwrap_or(serde_json::Value::Null);
        snapshot
    }
}

#[cfg(test)]
mod flow_session_escape_origin {
    use super::{flow_session_returns_to_desk, remove_flow_session, AppView};

    fn desk_view() -> AppView {
        AppView::FlowUxView {
            variant: crate::flows::model::FlowUxVariant::Flash,
            filter: String::new(),
            selected_index: 0,
            inline_run: None,
        }
    }

    /// Main-menu-launched flow sessions must escape straight back through
    /// `go_back_or_close` (main menu → hide), never detour through the
    /// Conversation Desk the user never visited. Regression lock for the
    /// 2026-07-10 report: launcher → flow chat → Escape required an extra
    /// Escape on the empty main menu because the session backgrounded to
    /// the desk first.
    #[test]
    fn main_menu_launch_does_not_return_to_desk() {
        assert!(!flow_session_returns_to_desk(&AppView::ScriptList, false));
        // Even a stale previously-captured desk origin must not leak into a
        // session entered from the launcher.
        assert!(!flow_session_returns_to_desk(&AppView::ScriptList, true));
    }

    #[test]
    fn desk_launch_returns_to_desk() {
        assert!(flow_session_returns_to_desk(&desk_view(), false));
    }

    #[test]
    fn session_to_session_switch_keeps_captured_origin() {
        let session = AppView::FlowSessionView { session_id: 7 };
        assert!(flow_session_returns_to_desk(&session, true));
        assert!(!flow_session_returns_to_desk(&session, false));
    }

    #[test]
    fn terminate_removes_session_even_with_turn_in_flight() {
        use crate::flows::session::{ActiveTurn, FlowSessionMeta, SessionState, SessionTransport};
        let meta = FlowSessionMeta {
            id: 7,
            flow_id: "project:test".into(),
            flow_name: "flow-test".into(),
            friendly_name: "Test".into(),
            origin: "Project".into(),
            engine: "codex".into(),
            flow_path: "/tmp/flow-test.md".into(),
            flow_mtime_ms: 0,
            cwd: "/tmp".into(),
            transport: SessionTransport::CodexThread,
            state: SessionState::Working,
            started_at: std::time::Instant::now(),
            turns: vec![],
            active_turn: Some(ActiveTurn {
                run_id: None,
                message_id: "message".into(),
                assistant_acc: String::new(),
                current_item_id: None,
                item_acc: String::new(),
                user_text: "hello".into(),
            }),
            thread_ready: true,
            needs_rethread: false,
        };
        let mut sessions = vec![(meta, ())];
        let removed = remove_flow_session(&mut sessions, 7).expect("live session removed");
        assert!(
            removed.0.active_turn.is_some(),
            "mid-turn termination is allowed"
        );
        assert!(sessions.is_empty());
    }
}
