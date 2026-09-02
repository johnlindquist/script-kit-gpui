impl ScriptListApp {
    pub(crate) fn create_owned_flow_session(&mut self, flow: &crate::flows::model::FlowDescriptor, cx: &mut Context<Self>) -> anyhow::Result<u64> {
        crate::runtime_policy::WindowHostPolicy::OwnedHidden.validate()?;
        let id = self.create_flow_session(flow, cx);
        self.open_flow_session(id, cx);
        Ok(id)
    }

    pub(crate) fn apply_owned_flow_event(
        &mut self, session_id: u64, expected_message_id: &str,
        event: crate::flows::codex_client::FlowThreadEvent, cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        crate::runtime_policy::WindowHostPolicy::OwnedHidden.validate()?;
        let index = self.flow_session_index(session_id).ok_or_else(|| anyhow::anyhow!("flow_session_missing"))?;
        let active = self.conversations.flow_sessions[index].0.active_turn.as_ref()
            .ok_or_else(|| anyhow::anyhow!("flow_turn_missing"))?;
        anyhow::ensure!(active.message_id == expected_message_id, "stale_flow_turn");
        use crate::flows::codex_client::FlowThreadEvent;
        let event_session = match &event {
            FlowThreadEvent::ThreadStarted { session_id, .. } | FlowThreadEvent::TurnStarted { session_id }
            | FlowThreadEvent::AgentDelta { session_id, .. } | FlowThreadEvent::AgentMessageFinal { session_id, .. }
            | FlowThreadEvent::TurnCompleted { session_id, .. } | FlowThreadEvent::TurnFailed { session_id, .. }
            | FlowThreadEvent::SessionFailed { session_id, .. } => *session_id,
        };
        anyhow::ensure!(event_session == session_id, "wrong_flow_session_event");
        self.apply_flow_thread_event(event, cx);
        cx.notify();
        Ok(())
    }
    fn flow_session_index(&self, session_id: u64) -> Option<usize> {
        self.conversations
            .flow_sessions
            .iter()
            .position(|(meta, _)| meta.id == session_id)
    }

    /// Whether a turn is in flight on this session.
    ///
    /// A session that no longer exists reads as NOT working, so a stale id
    /// never suppresses an affordance on a live conversation.
    pub(crate) fn flow_session_has_active_turn(&self, session_id: u64) -> bool {
        self.flow_session_index(session_id).is_some_and(|index| {
            self.conversations.flow_sessions[index]
                .0
                .active_turn
                .is_some()
        })
    }

    pub(crate) fn flow_session_archives(&self, session_id: u64) -> Vec<(String, usize)> {
        self.flow_session_index(session_id)
            .map(|index| {
                self.conversations.flow_sessions[index]
                    .0
                    .archived_threads
                    .iter()
                    .map(|thread| (thread.id.clone(), thread.turns.len()))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn flow_conversation_command_facts(
        &self,
        session_id: u64,
    ) -> crate::components::conversation_actions::FlowConversationCommandFacts {
        let Some(index) = self.flow_session_index(session_id) else {
            return Default::default();
        };
        let meta = &self.conversations.flow_sessions[index].0;
        crate::components::conversation_actions::FlowConversationCommandFacts {
            response_in_progress: meta.active_turn.is_some(),
            viewing_archive: meta.selected_is_archived(),
            has_archives: !meta.archived_threads.is_empty(),
            selected_has_response: meta
                .selected_turns()
                .iter()
                .any(|turn| !turn.assistant.trim().is_empty()),
            composer_has_text: !self.filter_text.trim().is_empty(),
            hidden_draft_exists: meta.selected_is_archived() && !meta.active_draft.is_empty(),
            runtime_attached: meta.thread_ready,
        }
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
        let descriptor = self.flow_desk_row_descriptor(&row);
        let verb = if run_once {
            descriptor.secondary
        } else {
            Some(descriptor.primary)
        };
        let Some(verb) = verb else {
            return;
        };

        match (&row, verb) {
            (FlowDeskRow::Session(session_id), FlowDeskRowVerb::OpenConversation) => {
                self.open_flow_session(*session_id, cx);
            }
            (FlowDeskRow::Run(_), FlowDeskRowVerb::OpenRunActions) => {
                self.toggle_flow_desk_actions(window, cx);
            }
            (FlowDeskRow::Flow(flow), FlowDeskRowVerb::OpenInTerminal) => {
                self.open_flow_in_terminal(flow, cx);
            }
            (FlowDeskRow::Flow(flow), FlowDeskRowVerb::RunOnce) => {
                self.flow_desk_run_once(flow, cx);
            }
            (FlowDeskRow::Flow(flow), FlowDeskRowVerb::Converse) => {
                let trimmed = filter.trim();
                let lowered = trimmed.to_lowercase();
                let is_name_lookup = trimmed.is_empty()
                    || !trimmed.contains(' ')
                    || flow.name.to_lowercase() == lowered
                    || flow.friendly_name().to_lowercase() == lowered;
                let first_message = (!is_name_lookup).then(|| trimmed.to_string());
                self.resume_or_start_flow_session(flow, first_message, cx);
            }
            (FlowDeskRow::InstallMdflow, FlowDeskRowVerb::InstallMdflow) => {
                self.open_quick_terminal_with_command(None, "npm i -g mdflow".to_string(), cx);
            }
            (FlowDeskRow::UpgradeMdflow, FlowDeskRowVerb::UpgradeMdflow) => {
                self.open_quick_terminal_with_command(
                    None,
                    "npm i -g mdflow@latest".to_string(),
                    cx,
                );
            }
            (FlowDeskRow::RetryRoster, FlowDeskRowVerb::RetryRoster) => {
                let cwd = self.flow_ux_cwd();
                crate::flows::catalog::flow_catalog().refresh(&cwd);
                self.start_flow_ux_tick(cx);
            }
            (FlowDeskRow::ClearQuery, FlowDeskRowVerb::ClearSearch) => {
                self.clear_builtin_view_filter(cx);
            }
            (FlowDeskRow::InitFlows, FlowDeskRowVerb::ScaffoldFlows) => {
                let cwd = self.flow_ux_cwd();
                self.open_quick_terminal_with_command(
                    Some(std::path::PathBuf::from(cwd)),
                    "md init".to_string(),
                    cx,
                );
            }
            (FlowDeskRow::CreateFlow, FlowDeskRowVerb::CreateFlow) => {
                self.start_flow_create_session(cx);
            }
            _ => {
                tracing::warn!(
                    target: "script_kit::flows",
                    row = %descriptor.semantic_id,
                    verb = ?verb,
                    "Flow desk descriptor/activation mismatch"
                );
                return;
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
        if let Some(index) = self
            .conversations
            .flow_sessions
            .iter()
            .rposition(|(meta, _)| {
                meta.flow_id == flow.id && meta.flow_path == flow.path && meta.state.is_live()
            })
        {
            let (session_id, went_stale) = {
                let meta = &mut self.conversations.flow_sessions[index].0;
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
                    self.conversations.flow_sessions[index].0.transport,
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
                let result = self.submit_flow_chat_message(session_id, message.clone(), cx);
                self.stage_unconsumed_flow_message(message, result, cx);
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
        let entity = self.conversations.flow_sessions[index].1.clone();
        // ONE canonical conversion (WP-A4): migrate/normalize the snapshot
        // into canonical turns, then render AND store from that same vector,
        // so the restored session is semantically identical to the live one.
        let turns = crate::flows::session::canonical_session_turns(&snapshot);
        let active_thread = snapshot
            .threads
            .iter()
            .find(|thread| thread.id == snapshot.active_thread_id)
            .cloned();
        let archived_threads: Vec<crate::flows::session::FlowArchivedThread> = snapshot
            .threads
            .iter()
            .filter(|thread| {
                thread.state == crate::flows::session::PersistedFlowThreadState::Archived
            })
            .map(|thread| crate::flows::session::FlowArchivedThread {
                id: thread.id.clone(),
                parent_thread_id: thread.parent_thread_id.clone(),
                created_at: thread.created_at.clone(),
                archived_at: thread
                    .archived_at
                    .clone()
                    .unwrap_or_else(|| snapshot.saved_at.clone()),
                inherited_turn_count: thread.inherited_turn_count,
                turns: crate::flows::session::canonical_persisted_turns(
                    snapshot.version,
                    &thread.turns,
                ),
            })
            .collect();
        entity.update(cx, |chat, cx| {
            for (turn_index, turn) in turns.iter().enumerate() {
                chat.add_message(
                    crate::protocol::ChatPromptMessage::user(turn.user.clone()),
                    cx,
                );
                let display = flow_turn_display_assistant(turn);
                let failed = turn.outcome == crate::flows::session::PersistedTurnOutcome::Failed;
                if !display.is_empty() || failed {
                    let message_id = format!("flow-{session_id}-restored-turn-{turn_index}");
                    chat.add_message(
                        crate::protocol::ChatPromptMessage::assistant(display)
                            .with_id(message_id.clone()),
                        cx,
                    );
                    if failed {
                        let restored = turn.failure.clone().unwrap_or_else(
                            crate::flows::session::PersistedAiFailure::unknown_default,
                        );
                        chat.set_message_failure(
                            &message_id,
                            restored.to_failure(),
                            restored.safe_summary,
                            cx,
                        );
                    }
                }
            }
        });
        let meta = &mut self.conversations.flow_sessions[index].0;
        meta.active_thread_id = snapshot.active_thread_id.clone();
        meta.active_thread_created_at = active_thread
            .as_ref()
            .map(|thread| thread.created_at.clone())
            .unwrap_or_else(|| snapshot.saved_at.clone());
        meta.active_parent_thread_id = active_thread
            .as_ref()
            .and_then(|thread| thread.parent_thread_id.clone());
        meta.inherited_turn_count = active_thread
            .as_ref()
            .map(|thread| thread.inherited_turn_count)
            .unwrap_or(0);
        meta.persistence_revision = snapshot.revision.max(1);
        meta.turns = turns;
        meta.archived_threads = archived_threads;
        meta.transcript_selection = crate::flows::session::FlowTranscriptSelection::Active;
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
            let result = self.submit_flow_chat_message(session_id, message.clone(), cx);
            self.stage_unconsumed_flow_message(message, result, cx);
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
            let result = self.submit_flow_chat_message(session_id, message.clone(), cx);
            self.stage_unconsumed_flow_message(message, result, cx);
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
        crate::flows::remember_flow_cwd(&cwd);
        self.conversations.flow_session_counter += 1;
        let session_id = self.conversations.flow_session_counter;
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
            std::sync::Arc::new(move |request| {
                submit_sender.try_send(crate::flows::session::FlowChatRequest::Submit {
                    session_id, text: request.outbound_text().to_string(),
                }).map_err(|_| "flow_submission_channel_full_or_closed".to_string())
            });
        // The flow session (this view) is the SINGLE lifecycle/key owner:
        // Esc backgrounds and Enter submits the shared draft; runtime
        // termination remains an Actions-only confirmed command. The hosted
        // ChatPrompt runs as a pure transcript body (TranscriptOnly), so it
        // installs no key handlers and needs no escape callback of its own.
        let mut chat = crate::prompts::ChatPrompt::new(
            format!("flow-session-{session_id}"),
            Some(format!("Message {friendly}…")),
            vec![],
            None,
            None,
            self.focus_handle.clone(),
            Some(submit_callback),
            std::sync::Arc::clone(&self.theme),
        )
        .with_title(friendly.clone())
        .with_save_history(false)
        .with_host_mode(crate::prompts::ChatPromptHostMode::TranscriptOnly {
            alignment: crate::prompts::ChatTranscriptAlignment::Top,
        })
        .with_empty_state_note(
            flow.description
                .clone()
                .unwrap_or_else(|| format!("Converse with {friendly}.")),
        );
        let actions_sender = self.flow_chat_sender.clone();
        chat.set_on_show_actions(std::sync::Arc::new(move |_id: String| {
            let _ = actions_sender
                .try_send(crate::flows::session::FlowChatRequest::ShowActions { session_id });
        }));
        // S10: without a recovery callback the per-turn card renders only
        // CopyDetails — every action that could actually fix the failure is
        // hidden, because `turn_recovery_capabilities` derives availability
        // from `on_recovery.is_some()`. The session owner can perform them,
        // so route them back to it.
        let recovery_sender = self.flow_chat_sender.clone();
        let chat = chat.with_recovery_binding(crate::prompts::ChatPromptRecoveryBinding {
            capabilities: crate::ai::reliability::SurfaceRecoveryCapabilities::only([
                sk_protocol::ai_reliability::RecoveryActionKind::RethreadFlow,
                sk_protocol::ai_reliability::RecoveryActionKind::RepairComponent,
            ]),
            callback: std::sync::Arc::new(
                move |message_id: String, action: sk_protocol::ai_reliability::AiRecoveryAction| {
                    recovery_sender.try_send(crate::flows::session::FlowChatRequest::Recovery {
                        session_id, message_id, action,
                    }).map_err(|_| Box::new(crate::ai::reliability::runtime_closed_failure(
                        sk_protocol::ai_reliability::ProtocolComponent::Codex,
                        "flow_recovery_channel_full_or_closed",
                    )))
                },
            ),
        });
        let entity = cx.new(|_| chat);
        let definition_model = std::fs::read_to_string(&flow.path).ok().and_then(|source| {
            crate::flows::session::resolve_flow_thread_contract(&source, "")
                .profile
                .model
        });
        let origin_kind = match flow.source {
            crate::flows::model::FlowSource::Project => {
                crate::flows::session::FlowOriginKind::Project
            }
            crate::flows::model::FlowSource::Package => {
                crate::flows::session::FlowOriginKind::Package
            }
            crate::flows::model::FlowSource::Global => {
                crate::flows::session::FlowOriginKind::Global
            }
            crate::flows::model::FlowSource::Registry => {
                crate::flows::session::FlowOriginKind::BuiltIn
            }
        };

        let meta = crate::flows::session::FlowSessionMeta {
            id: session_id,
            flow_id: flow.id.clone(),
            flow_name: flow.name.clone(),
            friendly_name: friendly,
            origin: flow.origin_label().to_string(),
            origin_kind,
            engine: flow.engine.clone(),
            model_source: if definition_model.is_some() {
                crate::flows::session::FlowModelSource::Definition
            } else {
                crate::flows::session::FlowModelSource::Unavailable
            },
            model: definition_model,
            flow_path: flow.path.clone(),
            flow_mtime_ms: crate::flows::session::flow_definition_mtime_ms(&flow.path),
            cwd,
            transport,
            state: crate::flows::session::SessionState::NeedsYou,
            started_at: std::time::Instant::now(),
            last_activity: std::time::SystemTime::now(),
            active_thread_id: crate::flows::session::FlowSessionMeta::new_thread_id(),
            active_thread_created_at: chrono::Utc::now().to_rfc3339(),
            active_parent_thread_id: None,
            turns: Vec::new(),
            archived_threads: Vec::new(),
            transcript_selection: crate::flows::session::FlowTranscriptSelection::Active,
            inherited_turn_count: 0,
            active_draft: String::new(),
            draft_generation: 0,
            runtime_generation: 0,
            persistence_revision: 0,
            active_turn: None,
            thread_ready: !matches!(
                transport,
                crate::flows::session::SessionTransport::CodexThread
            ),
            needs_rethread: false,
            pending_runtime_termination: false,
            reliability: crate::flows::session::FlowReliability::new(
                &flow.id,
                &flow.path,
                &flow.engine,
            ),
        };
        // Codex transport: warm the protocol thread while the user types
        // their first message. File read + server spawn + thread/start all
        // happen off the GPUI thread; failures surface as SessionFailed
        // through the normal event drain.
        if !crate::runtime_policy::is_owned_evaluation() && matches!(
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
        self.conversations.flow_sessions.push((meta, entity));
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
    ) -> FlowChatSubmitResult {
        let Some(index) = self.flow_session_index(session_id) else {
            return FlowChatSubmitResult::MissingSession;
        };
        let text = text.trim().to_string();
        if text.is_empty() {
            return FlowChatSubmitResult::Empty;
        }
        if self.conversations.flow_sessions[index]
            .0
            .selected_is_archived()
        {
            self.toast_manager.push(
                crate::components::toast::Toast::info(
                    "Archived conversations are read-only — use Continue as New".to_string(),
                    &self.theme,
                )
                .duration_ms(Some(2500)),
            );
            cx.notify();
            return FlowChatSubmitResult::ReadOnlyArchive;
        }
        // Busy check runs BEFORE any input/transcript mutation so a rejected
        // submit leaves the composer draft untouched for the caller to keep.
        if self.conversations.flow_sessions[index]
            .0
            .active_turn
            .is_some()
        {
            self.toast_manager.push(
                crate::components::toast::Toast::error(
                    "Still working — stop the current turn first (⌘K)".to_string(),
                    &self.theme,
                )
                .duration_ms(Some(2500)),
            );
            cx.notify();
            return FlowChatSubmitResult::Busy;
        }

        let mut thread_profile: Option<crate::flows::session::FlowThreadProfile> = None;
        let mut flow_unreadable: Option<String> = None;
        let (transport, prompt) = {
            let meta = &self.conversations.flow_sessions[index].0;
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

        let turn_index = self.conversations.flow_sessions[index].0.turns.len();
        let message_id = format!("flow-{session_id}-turn-{turn_index}");
        let entity = self.conversations.flow_sessions[index].1.clone();
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
            let turn_ordinal = meta.turns.len();
            let (flow_id, flow_path) = (meta.flow_id.clone(), meta.flow_path.clone());
            meta.reliability
                .begin_turn(&flow_id, &flow_path, turn_ordinal);
            // Classified as invalid configuration: the definition itself —
            // not the provider — is what blocks the turn. The raw path/io
            // detail stops at the diagnostic vault.
            let failure = crate::ai::reliability::provider_failure(
                sk_protocol::ai_reliability::ProtocolComponent::Mdflow,
                format!("invalid configuration: {error}"),
            );
            self.finish_flow_turn(
                session_id,
                crate::flows::session::SessionState::Done(None),
                FlowTurnOutcome::Failed(failure),
                cx,
            );
            cx.notify();
            // The message was consumed into a failed-closed turn, so the
            // composer draft should still clear.
            return FlowChatSubmitResult::FailedClosed;
        }

        tracing::info!(
            target: "script_kit::flows",
            event = "flow_turn_submit",
            session_id,
            transport = ?transport,
            prompt_len = prompt.len(),
            "Submitting flow turn"
        );

        // Install the active turn and Working state BEFORE backend dispatch so
        // even a synchronously queued failure event observes a valid turn
        // (Oracle audit 2026-07-21). The mdflow run id is filled in after
        // launch, before returning to the event loop.
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
        let turn_ordinal = meta.turns.len();
        let (flow_id, flow_path) = (meta.flow_id.clone(), meta.flow_path.clone());
        meta.reliability
            .begin_turn(&flow_id, &flow_path, turn_ordinal);

        if crate::runtime_policy::is_owned_evaluation() {
            // Runtime acceptance is real; subsequent controlled events enter
            // apply_flow_thread_event and finish_flow_turn, never a provider.
            cx.notify();
            return FlowChatSubmitResult::Dispatched;
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
                        // Conversation turn: stream from the append-only capture,
                        // never the bounded display tail (cursor corruption P0).
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
        FlowChatSubmitResult::Dispatched
    }

    /// The ONE draft-consumption transaction for a flow session: submit the
    /// current main-input draft and clear it only when the submit consumed it.
    /// Both the keyboard Enter handler and the native footer Send button MUST
    /// route through this method — a caller that clears first destroys the
    /// draft on a Busy race (Oracle audit 2026-07-21, Footer-B).
    pub(crate) fn submit_flow_session_draft(
        &mut self,
        session_id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> FlowChatSubmitResult {
        self.capture_flow_session_draft(session_id);
        let draft = self.filter_text.clone();
        let result = self.submit_flow_chat_message(session_id, draft, cx);
        if result.consumes_draft() {
            // A sent prompt starts recall over. Leaving the cursor parked
            // would make the next Up jump to the second-newest prompt and
            // silently skip the one just sent, and a stale parked draft would
            // reappear later over text the user had moved on from.
            self.flow_session_prompt_history_index = None;
            self.flow_session_prompt_draft = None;
            self.clear_flow_session_draft(session_id);
            self.set_filter_text_immediate(String::new(), window, cx);
        }
        result
    }

    /// A routed `initial_message` that could not be submitted (session busy,
    /// or vanished) must NOT be silently dropped: stage it as the main-input
    /// draft so the user's message is waiting in the composer (Oracle audit
    /// 2026-07-21: a busy reattach used to swallow the routed message).
    fn stage_unconsumed_flow_message(
        &mut self,
        message: String,
        result: FlowChatSubmitResult,
        cx: &mut Context<Self>,
    ) {
        if result.consumes_draft() || matches!(result, FlowChatSubmitResult::Empty) {
            return;
        }
        tracing::warn!(
            target: "script_kit::flows",
            event = "flow_initial_message_staged_as_draft",
            result = ?result,
            "Initial flow message could not submit — staged as composer draft"
        );
        self.filter_text = crate::components::text_input::normalize_single_line_text(message);
        self.pending_filter_sync = true;
        if let AppView::FlowSessionView { session_id } = self.current_view {
            self.capture_flow_session_draft(session_id);
        }
        cx.notify();
    }

    /// Append streamed assistant text to a session's open turn. The single
    /// visible-commit helper for a session's child ChatPrompt: both streamed
    /// deltas and the finalized display suffix route through here so they count
    /// identically (WP-B3). Empty deltas are rejected before any commit count.
    fn append_flow_turn_text(&mut self, session_id: u64, delta: &str, cx: &mut Context<Self>) {
        // WP-B3: reject an empty child delta before counting — an empty commit
        // is not effective work and must not inflate the child-commit rate.
        if delta.is_empty() {
            return;
        }
        let Some(index) = self.flow_session_index(session_id) else {
            return;
        };
        let entity = self.conversations.flow_sessions[index].1.clone();
        let Some(active) = self.conversations.flow_sessions[index]
            .0
            .active_turn
            .as_mut()
        else {
            return;
        };
        active.assistant_acc.push_str(delta);
        active.item_acc.push_str(delta);
        let message_id = active.message_id.clone();
        let delta = delta.to_string();
        // WP-B3: a non-empty text delta committed into the child ChatPrompt
        // entity, plus its byte count. An effective flow event.
        crate::chat_hot_counters::record_flow_child_commit(delta.len());
        crate::chat_hot_counters::record_flow_event_effective();
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
            let Some(active) = self.conversations.flow_sessions[index]
                .0
                .active_turn
                .as_mut()
            else {
                return;
            };
            active.enter_item(item_id)
        };
        if needs_break {
            self.append_flow_turn_text(session_id, "\n\n", cx);
            // The break belongs to the boundary, not the new item's text.
            if let Some(active) = self.conversations.flow_sessions[index]
                .0
                .active_turn
                .as_mut()
            {
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
        let Some(active) = self.conversations.flow_sessions[index].0.active_turn.take() else {
            return;
        };
        // A turn reaching a terminal state is semantic activity (Oracle step
        // 5). Streamed tokens deliberately never touch: per-token updates
        // would reorder the list continuously mid-answer.
        self.conversations.flow_sessions[index].0.touch_now();
        let entity = self.conversations.flow_sessions[index].1.clone();
        let message_id = active.message_id.clone();
        // Drive the reducer-owned reliability state BEFORE projecting the
        // transcript so recovery actions are already truthful when the card
        // renders in the same pass (S09).
        let had_partial_output = !active.assistant_acc.is_empty();
        match &outcome {
            FlowTurnOutcome::Ok => {
                self.conversations.flow_sessions[index]
                    .0
                    .reliability
                    .complete_turn();
            }
            FlowTurnOutcome::Stopped => {
                self.conversations.flow_sessions[index]
                    .0
                    .reliability
                    .cancel_turn(had_partial_output);
            }
            FlowTurnOutcome::Failed(record) => {
                self.conversations.flow_sessions[index]
                    .0
                    .reliability
                    .fail_turn(record.failure.clone());
            }
        }
        let failure_for_row = match &outcome {
            FlowTurnOutcome::Failed(record) => Some(record.failure.clone()),
            FlowTurnOutcome::Ok | FlowTurnOutcome::Stopped => None,
        };
        // Build the finalized turn ONCE: raw assistant + structured outcome
        // for persistence/rollup, plus the exact display suffix for the live
        // row. Both sides come from the same projection (WP-A3).
        let FinalizedFlowTurn { turn, live_suffix } = finalize_flow_session_turn(active, outcome);
        let failure_note = turn
            .failure
            .as_ref()
            .map(|failure| failure.safe_summary.clone());
        let had_error = failure_note.is_some();
        // WP-B3: the finalized display suffix is a visible commit too — count it
        // through the same child-commit helper semantics as streamed deltas.
        if !live_suffix.is_empty() {
            crate::chat_hot_counters::record_flow_child_commit(live_suffix.len());
        }
        entity.update(cx, |chat, cx| {
            // append_chunk is gated on the live stream, so project any
            // finalized caption before closing the same assistant row.
            if !live_suffix.is_empty() {
                chat.append_chunk(&message_id, &live_suffix, cx);
            }
            chat.complete_streaming(&message_id, cx);
            if let (Some(failure), Some(note)) = (failure_for_row, failure_note) {
                chat.set_message_failure(&message_id, failure, note, cx);
            }
        });
        let meta = &mut self.conversations.flow_sessions[index].0;
        meta.turns.push(turn);
        meta.state = state;
        let terminate_runtime = meta.pending_runtime_termination;
        if terminate_runtime {
            meta.pending_runtime_termination = false;
            meta.thread_ready = false;
            meta.needs_rethread = true;
            meta.runtime_generation = meta.runtime_generation.saturating_add(1);
        }
        // Snapshot the conversation through the FIFO store so an app restart
        // can restore it. Enqueued synchronously: on-disk order always
        // matches the order turns settled (WP-A1 — detached per-turn threads
        // let an older snapshot overwrite a newer one).
        crate::flows::session::conversation_store()
            .persist_snapshot(meta.next_persisted_snapshot());
        if terminate_runtime {
            crate::flows::codex_client::codex_app_server().forget_session(session_id);
        }
        tracing::info!(
            target: "script_kit::flows",
            event = "flow_turn_settled",
            session_id,
            state = state.label(),
            had_error,
            "Flow turn settled"
        );
        // WP-B3: a settled turn is an effective state transition.
        crate::chat_hot_counters::record_flow_event_effective();
        // WP5: settle boundary — publish a fresh counter reading when a flow
        // turn finalizes so a probe never races the throttled tick snapshot.
        crate::chat_hot_counters::log_snapshot("flow_turn_settled");
    }

    /// Apply one codex app-server event to its session.
    fn apply_flow_thread_event(
        &mut self,
        event: crate::flows::codex_client::FlowThreadEvent,
        cx: &mut Context<Self>,
    ) {
        use crate::flows::codex_client::FlowThreadEvent;
        use crate::flows::session::SessionState;
        // WP-B3: one codex app-server transport event pulled off the stream
        // (ingress). Effective mutations are counted at the sites that actually
        // change session state (child commits, turn settle), so an empty/no-op
        // event never counts as effective work.
        crate::chat_hot_counters::record_flow_event_received();
        match event {
            FlowThreadEvent::ThreadStarted { session_id, model } => {
                if let Some(index) = self.flow_session_index(session_id) {
                    let meta = &mut self.conversations.flow_sessions[index].0;
                    meta.thread_ready = true;
                    meta.needs_rethread = false;
                    if !model.is_empty() {
                        meta.model = Some(model);
                        meta.model_source = crate::flows::session::FlowModelSource::Runtime;
                    }
                }
            }
            FlowThreadEvent::TurnStarted { session_id } => {
                if let Some(index) = self.flow_session_index(session_id) {
                    if self.conversations.flow_sessions[index]
                        .0
                        .active_turn
                        .is_some()
                    {
                        self.conversations.flow_sessions[index].0.state = SessionState::Working;
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
                let item_acc = self.conversations.flow_sessions[index]
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
                outcome,
            } => {
                let (state, outcome) = match outcome {
                    crate::ai::reliability::AiTurnRuntimeOutcome::Completed { .. } => {
                        (SessionState::NeedsYou, FlowTurnOutcome::Ok)
                    }
                    crate::ai::reliability::AiTurnRuntimeOutcome::Cancelled { .. } => {
                        (SessionState::NeedsYou, FlowTurnOutcome::Stopped)
                    }
                };
                self.finish_flow_turn(session_id, state, outcome, cx);
            }
            FlowThreadEvent::TurnFailed {
                session_id,
                failure,
            } => {
                self.finish_flow_turn(
                    session_id,
                    SessionState::Done(None),
                    FlowTurnOutcome::Failed(failure),
                    cx,
                );
            }
            FlowThreadEvent::SessionFailed {
                session_id,
                failure,
            } => {
                let Some(index) = self.flow_session_index(session_id) else {
                    return;
                };
                // The protocol thread is gone (server death or thread/start
                // failure). The next submit must re-thread with the flow's
                // contract + transcript rollup, and the footer must show
                // Connecting again instead of pretending the thread lives.
                self.conversations.flow_sessions[index].0.thread_ready = false;
                self.conversations.flow_sessions[index].0.needs_rethread = true;
                if self.conversations.flow_sessions[index]
                    .0
                    .active_turn
                    .is_some()
                {
                    self.finish_flow_turn(
                        session_id,
                        crate::flows::session::SessionState::Done(None),
                        FlowTurnOutcome::Failed(failure),
                        cx,
                    );
                } else {
                    // Engine death while idle: no turn to settle, but the
                    // typed recovery state must still become actionable.
                    self.conversations.flow_sessions[index]
                        .0
                        .reliability
                        .fail_outside_turn(failure.failure.clone());
                    self.conversations.flow_sessions[index].0.state =
                        crate::flows::session::SessionState::Done(None);
                    cx.notify();
                }
            }
        }
    }

    /// Stream mdflow-turn run output into transcripts and settle finished
    /// turns. Returns true when anything changed.
    fn sync_mdflow_turns(&mut self, cx: &mut Context<Self>) -> bool {
        let registry = crate::flows::run_registry::flow_run_registry();
        let mut dirty = false;
        for index in 0..self.conversations.flow_sessions.len() {
            // WP-B3: one session scanned per sync pass (the O(sessions) walk).
            crate::chat_hot_counters::record_flow_session_scanned();
            let (session_id, run_id, acc_len) = {
                let meta = &self.conversations.flow_sessions[index].0;
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
                    FlowTurnOutcome::Failed(crate::ai::reliability::process_failure(
                        sk_protocol::ai_reliability::ProtocolComponent::Mdflow,
                        crate::ai::reliability::ProcessFailureFacts::SessionLost { session: None },
                    )),
                    cx,
                );
                dirty = true;
                continue;
            };
            if mdflow_run_accepted_context(run.phase) {
                // A fast fixture can move Starting → Succeeded between ticks;
                // terminal success/cancellation is still authoritative proof
                // that mdflow accepted and started this context.
                let meta = &mut self.conversations.flow_sessions[index].0;
                meta.thread_ready = true;
                meta.needs_rethread = false;
            }
            // Stream from the append-only conversation capture. The bounded
            // display tail front-evicts, which broke the byte cursor on long
            // turns (silent stalls / garbled text — 2026-07-11 audit P0);
            // the capture never evicts, so `acc_len` is always a valid char
            // boundary within it.
            let full = run.conversation_stdout.clone().unwrap_or_default();
            if full.len() > acc_len {
                if let Some(delta) = full.get(acc_len..) {
                    let delta = delta.to_string();
                    // WP-B3: bytes copied out of the mdflow child's stdout
                    // capture into the transcript this tick.
                    crate::chat_hot_counters::record_flow_stdout_bytes_copied(delta.len());
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
                        FlowTurnOutcome::Failed(run.failure.clone().unwrap_or_else(|| {
                            crate::ai::reliability::process_failure(
                                sk_protocol::ai_reliability::ProtocolComponent::Mdflow,
                                crate::ai::reliability::ProcessFailureFacts::ChildExited {
                                    exit_code: run.exit_code.map(|code| code as i32),
                                    signal: None,
                                },
                            )
                        })),
                    ),
                };
                self.finish_flow_turn(session_id, state, outcome, cx);
                dirty = true;
            }
        }
        dirty
    }

    /// Show an existing session (same ChatPrompt entity — the reattach).
    /// Resume a backgrounded conversation from a main-menu Conversations row,
    /// dispatched by tagged session id to the EXACT retained entity in the
    /// [`BackgroundedSessionStore`] (spec §8 step 7).
    ///
    /// Dead rows are pruned, never rendered disabled: when the store no
    /// longer holds an entity for the id (or holds a wrong-kind entity —
    /// a store invariant violation), the record is removed with a typed
    /// diagnostic so the next paint drops the row.
    pub(crate) fn resume_backgrounded_conversation(
        &mut self,
        id: crate::ai::conversations::ConversationSessionId,
        cx: &mut Context<Self>,
    ) {
        use crate::ai::conversations::{ConversationSessionId, FlowSessionId, SessionEntity};

        match (&id, self.conversations.resume_entity(&id)) {
            (
                ConversationSessionId::Flow(FlowSessionId(session_id)),
                Some(SessionEntity::Flow(_)),
            ) => {
                // `open_flow_session` owns the semantic resume touch.
                self.open_flow_session(*session_id, cx);
            }
            (_, Some(SessionEntity::AgentChat(entity) | SessionEntity::QuickAi(entity))) => {
                // Explicit resume is semantic activity.
                self.conversations.touch(&id, std::time::SystemTime::now());
                self.enter_embedded_agent_chat_surface(entity, cx);
                cx.notify();
            }
            (_, None) | (_, Some(SessionEntity::Flow(_))) => {
                tracing::error!(
                    event = "conversation_row_dead_entity_pruned",
                    conversation_id = %id.automation_id(),
                    "Conversations row had no resumable entity; pruning the record"
                );
                let _ = self.conversations.remove(&id);
                cx.notify();
            }
        }
    }
}
