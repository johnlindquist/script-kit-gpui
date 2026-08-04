use super::*;
use serde::Serialize;
use smol::Timer;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_AGENT_CHAT_ENTRY_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

pub(crate) fn next_agent_chat_entry_request_id() -> String {
    format!(
        "agent-chat-entry-{}",
        NEXT_AGENT_CHAT_ENTRY_REQUEST_ID.fetch_add(1, Ordering::Relaxed)
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AgentChatEntryOrigin {
    MainLauncher,
    FileSearch,
    ActionsDialog,
    PluginSkill { skill_id: String },
    Notes,
    Dictation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AgentChatThreadTarget {
    ExistingDetachedOrEmbedded,
    CurrentHostEmbedded,
    FreshEmbedded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AgentChatEntryVerb {
    Open,
    Add,
    Continue,
    Ask,
    Send,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AgentChatEntryIntent {
    Open { seed_text: Option<String> },
    Add { seed_text: Option<String> },
    Continue { seed_text: Option<String> },
    Ask { text: String },
    Send { text: String },
}

impl AgentChatEntryIntent {
    pub(crate) fn open(seed_text: Option<String>) -> Self {
        Self::Open { seed_text }
    }

    pub(crate) fn add(seed_text: Option<String>) -> Self {
        Self::Add { seed_text }
    }

    pub(crate) fn continue_with(seed_text: Option<String>) -> Self {
        Self::Continue { seed_text }
    }

    pub(crate) fn ask(text: String) -> Result<Self, &'static str> {
        if text.trim().is_empty() {
            Err("ask_requires_nonempty_text")
        } else {
            Ok(Self::Ask { text })
        }
    }

    pub(crate) fn send(text: String) -> Result<Self, &'static str> {
        if text.trim().is_empty() {
            Err("send_requires_nonempty_text")
        } else {
            Ok(Self::Send { text })
        }
    }

    pub(crate) fn verb(&self) -> AgentChatEntryVerb {
        match self {
            Self::Open { .. } => AgentChatEntryVerb::Open,
            Self::Add { .. } => AgentChatEntryVerb::Add,
            Self::Continue { .. } => AgentChatEntryVerb::Continue,
            Self::Ask { .. } => AgentChatEntryVerb::Ask,
            Self::Send { .. } => AgentChatEntryVerb::Send,
        }
    }

    pub(crate) fn seed_text(&self) -> Option<&str> {
        match self {
            Self::Open { seed_text } | Self::Add { seed_text } | Self::Continue { seed_text } => {
                seed_text.as_deref()
            }
            Self::Ask { text } | Self::Send { text } => Some(text),
        }
    }

    pub(crate) fn requests_submission(&self) -> bool {
        matches!(self, Self::Ask { .. } | Self::Send { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AgentChatOpenDisposition {
    Blocked,
    Reused,
    OpenedFresh,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AgentChatSubmissionOutcome {
    NotRequested,
    Pending,
    Accepted,
    Refused { reason_code: &'static str },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentChatContextStageOutcome {
    pub(crate) item_kind: &'static str,
    pub(crate) staged: bool,
    pub(crate) reason_code: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentChatEntryOutcome {
    pub(crate) request_id: String,
    pub(crate) verb: AgentChatEntryVerb,
    pub(crate) disposition: AgentChatOpenDisposition,
    pub(crate) destination_host: Option<String>,
    pub(crate) destination_thread_id: Option<String>,
    pub(crate) destination_generation: Option<u64>,
    pub(crate) text_staged: bool,
    pub(crate) context: Vec<AgentChatContextStageOutcome>,
    pub(crate) submission: AgentChatSubmissionOutcome,
    pub(crate) blocked_reason: Option<&'static str>,
    pub(crate) return_route: Option<&'static str>,
}

#[derive(Debug)]
pub(crate) struct AgentChatEntryTicket {
    pub(crate) request_id: String,
    pub(crate) completion: async_channel::Receiver<AgentChatEntryOutcome>,
}

#[must_use = "entry dispatch must be observed so callers cannot claim Opened before staging/submission completes"]
#[derive(Debug)]
pub(crate) enum AgentChatEntryDispatch {
    Complete(AgentChatEntryOutcome),
    Pending(AgentChatEntryTicket),
}

impl AgentChatEntryOutcome {
    pub(crate) fn source_consumed(&self) -> bool {
        if self.blocked_reason.is_some() || self.disposition == AgentChatOpenDisposition::Blocked {
            return false;
        }
        match self.verb {
            AgentChatEntryVerb::Ask | AgentChatEntryVerb::Send => {
                self.submission == AgentChatSubmissionOutcome::Accepted
            }
            AgentChatEntryVerb::Add => self.context.iter().any(|item| item.staged),
            AgentChatEntryVerb::Open | AgentChatEntryVerb::Continue => self.text_staged,
        }
    }

    pub(crate) fn feedback_verb(&self) -> &'static str {
        if self.blocked_reason.is_some() || self.disposition == AgentChatOpenDisposition::Blocked {
            return "Refused";
        }
        match (&self.verb, &self.submission) {
            (AgentChatEntryVerb::Ask, AgentChatSubmissionOutcome::Accepted) => "Asked",
            (AgentChatEntryVerb::Send, AgentChatSubmissionOutcome::Accepted) => "Sent",
            (AgentChatEntryVerb::Ask | AgentChatEntryVerb::Send, _) => "Refused",
            (AgentChatEntryVerb::Add, _) => "Added",
            (AgentChatEntryVerb::Continue, _) => "Continued",
            (AgentChatEntryVerb::Open, _) => "Opened",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct AgentChatEntryRequest {
    pub(crate) request_id: String,
    pub(crate) origin: AgentChatEntryOrigin,
    pub(crate) target: AgentChatThreadTarget,
    pub(crate) intent: AgentChatEntryIntent,
    pub(crate) ui_variant: crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant,
    pub(crate) context_policy: AgentChatContextPolicy,
    pub(crate) return_origin: Option<AppView>,
}

impl AgentChatEntryRequest {
    /// Shared launcher constructor. Private on purpose: callers pick a policy
    /// through the named constructors below so a variant/policy mismatch can
    /// never be expressed as a constructor argument.
    fn main_launcher_internal(
        seed_text: Option<String>,
        ui_variant: crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant,
        context_policy: AgentChatContextPolicy,
    ) -> Self {
        let intent = if ui_variant
            == crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::QuickAi
        {
            match seed_text {
                Some(text) if !text.trim().is_empty() => AgentChatEntryIntent::Ask { text },
                _ => AgentChatEntryIntent::Open { seed_text: None },
            }
        } else {
            AgentChatEntryIntent::open(seed_text)
        };
        Self {
            request_id: next_agent_chat_entry_request_id(),
            origin: AgentChatEntryOrigin::MainLauncher,
            target: AgentChatThreadTarget::ExistingDetachedOrEmbedded,
            intent,
            ui_variant,
            context_policy,
            return_origin: None,
        }
    }

    /// Standard launcher entry that MAY inherit the currently selected launcher
    /// row as ambient/focused context.
    pub(crate) fn main_launcher(seed_text: Option<String>) -> Self {
        Self::main_launcher_internal(
            seed_text,
            crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard,
            AgentChatContextPolicy::AmbientOrFocused,
        )
    }

    /// Standard launcher entry that must NOT inherit the selected launcher row.
    pub(crate) fn clean_main_launcher(seed_text: Option<String>) -> Self {
        Self::main_launcher_internal(
            seed_text,
            crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard,
            AgentChatContextPolicy::SuppressFocused,
        )
    }

    /// Launcher entry whose context policy is derived exhaustively from the UI
    /// variant (Standard inherits, every nonstandard variant suppresses).
    pub(crate) fn main_launcher_with_variant(
        seed_text: Option<String>,
        ui_variant: crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant,
    ) -> Self {
        let context_policy = AgentChatContextPolicy::for_main_launcher_variant(ui_variant);
        Self::main_launcher_internal(seed_text, ui_variant, context_policy)
    }

    pub(crate) fn explicit_ask(
        origin: AgentChatEntryOrigin,
        text: String,
        ui_variant: crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant,
        context_policy: AgentChatContextPolicy,
        return_origin: Option<AppView>,
    ) -> Result<Self, &'static str> {
        let intent = AgentChatEntryIntent::ask(text)?;
        Ok(Self {
            request_id: next_agent_chat_entry_request_id(),
            origin,
            target: AgentChatThreadTarget::ExistingDetachedOrEmbedded,
            intent,
            ui_variant,
            context_policy,
            return_origin,
        })
    }

    pub(crate) fn explicit_send(
        origin: AgentChatEntryOrigin,
        text: String,
        context_policy: AgentChatContextPolicy,
        return_origin: Option<AppView>,
    ) -> Result<Self, &'static str> {
        let intent = AgentChatEntryIntent::send(text)?;
        Ok(Self {
            request_id: next_agent_chat_entry_request_id(),
            origin,
            target: AgentChatThreadTarget::ExistingDetachedOrEmbedded,
            intent,
            ui_variant: crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard,
            context_policy,
            return_origin,
        })
    }

    /// Quick-question entry: double-tap of the main hotkey (and any future
    /// "just open a clean chat" affordance).
    ///
    /// CONTRACT: this request must open Agent Chat with an EMPTY composer and
    /// NO context chips. In particular it must never inherit the launcher's
    /// auto-selected default row — the user double-tapped from anywhere, they
    /// did not pick that row. Regression reference (2026-07-10): double-tap
    /// routed through the chip-staging entry, so whatever happened to sit at
    /// `first_selectable_index` (a Brain Inbox capture, a flow row) was staged
    /// as an `@cmd:` chip and pre-filled into the composer. The unit tests
    /// below lock the suppression policy; do not weaken them.
    pub(crate) fn quick_question() -> Self {
        Self::main_launcher_internal(
            None,
            crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard,
            AgentChatContextPolicy::NoContext,
        )
    }

    /// Notes→main handoff: open (or reuse) the MAIN window's Agent Chat with
    /// the selected note staged as an explicit `@note` reference, leaving the
    /// Notes window open.
    ///
    /// CONTRACT: origin is `Notes`; target is main-host-only
    /// (`CurrentHostEmbedded` — never the detached chat window); the UI
    /// variant is Standard; nothing auto-submits; no ambient or implicit
    /// focused context is inherited; no seed text may displace the canonical
    /// `@note` prefill.
    pub(crate) fn notes(
        target: crate::ai::TabAiTargetContext,
        supplemental_parts: Vec<crate::ai::message_parts::AiContextPart>,
        source: &'static str,
    ) -> Self {
        Self {
            request_id: next_agent_chat_entry_request_id(),
            origin: AgentChatEntryOrigin::Notes,
            target: AgentChatThreadTarget::CurrentHostEmbedded,
            intent: AgentChatEntryIntent::add(None),
            ui_variant: crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard,
            context_policy: AgentChatContextPolicy::NotesHandoff {
                target,
                supplemental_parts,
                source,
            },
            return_origin: None,
        }
    }

    /// Promote a bounded Quick AI result into a fresh full Agent Chat without
    /// submitting it or inheriting ambient launcher context.
    pub(crate) fn quick_ai_handoff(seed_text: String) -> Self {
        Self {
            request_id: next_agent_chat_entry_request_id(),
            origin: AgentChatEntryOrigin::MainLauncher,
            target: AgentChatThreadTarget::FreshEmbedded,
            intent: AgentChatEntryIntent::continue_with(Some(seed_text)),
            ui_variant: crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard,
            context_policy: AgentChatContextPolicy::SuppressFocused,
            return_origin: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct AgentChatEntryBaseline {
    host: Option<&'static str>,
    thread_id: Option<String>,
    message_count: usize,
    context_count: usize,
}

struct AgentChatEntryObservedSnapshot {
    host: &'static str,
    thread_id: String,
    state: crate::protocol::AgentChatStateSnapshot,
}

#[derive(Clone, Debug)]
struct AgentChatEntryCompletionPlan {
    request_id: String,
    verb: AgentChatEntryVerb,
    seed_text: Option<String>,
    expects_context: bool,
    return_route: Option<&'static str>,
    baseline: AgentChatEntryBaseline,
}

impl ScriptListApp {
    fn agent_chat_entry_snapshot(&self, cx: &App) -> Option<AgentChatEntryObservedSnapshot> {
        let (host, entity) = if let AppView::AgentChatView { entity, .. } = &self.current_view {
            ("main", entity.clone())
        } else {
            (
                "detached",
                crate::ai::agent_chat::ui::chat_window::get_detached_agent_chat_view_entity()?,
            )
        };
        let view = entity.read(cx);
        let thread_id = view.thread()?.read(cx).ui_thread_id().to_string();
        Some(AgentChatEntryObservedSnapshot {
            host,
            thread_id,
            state: view.collect_agent_chat_state_snapshot(cx),
        })
    }

    fn agent_chat_entry_baseline(&self, cx: &App) -> AgentChatEntryBaseline {
        self.agent_chat_entry_snapshot(cx).map_or_else(
            AgentChatEntryBaseline::default,
            |observed| AgentChatEntryBaseline {
                host: Some(observed.host),
                thread_id: Some(observed.thread_id),
                message_count: observed.state.message_count,
                context_count: observed.state.context_chip_count,
            },
        )
    }

    fn resolved_agent_chat_entry_outcome(
        &self,
        plan: &AgentChatEntryCompletionPlan,
        timed_out: bool,
        cx: &App,
    ) -> Option<AgentChatEntryOutcome> {
        let observed = self.agent_chat_entry_snapshot(cx)?;
        let same_destination = plan.baseline.host == Some(observed.host)
            && plan.baseline.thread_id.as_deref() == Some(observed.thread_id.as_str());
        let submitted = if same_destination {
            observed.state.message_count > plan.baseline.message_count
        } else {
            observed.state.message_count > 0
        };
        let text_staged = match plan.seed_text.as_deref() {
            None => true,
            Some(seed) if plan.verb == AgentChatEntryVerb::Ask || plan.verb == AgentChatEntryVerb::Send => submitted,
            Some(seed) => observed.state.input_text == seed,
        };
        let context_staged = observed.state.context_chip_count > plan.baseline.context_count;
        let submission = if matches!(plan.verb, AgentChatEntryVerb::Ask | AgentChatEntryVerb::Send)
        {
            if submitted {
                AgentChatSubmissionOutcome::Accepted
            } else if observed.state.status == "error" || observed.state.status == "setup" || timed_out {
                AgentChatSubmissionOutcome::Refused {
                    reason_code: if timed_out { "entry_timeout" } else { "runtime_refused" },
                }
            } else {
                return None;
            }
        } else {
            AgentChatSubmissionOutcome::NotRequested
        };
        if !text_staged && !timed_out {
            return None;
        }
        Some(AgentChatEntryOutcome {
            request_id: plan.request_id.clone(),
            verb: plan.verb,
            disposition: if same_destination {
                AgentChatOpenDisposition::Reused
            } else {
                AgentChatOpenDisposition::OpenedFresh
            },
            destination_host: Some(observed.host.to_string()),
            destination_thread_id: Some(observed.thread_id),
            destination_generation: Some(self.tab_ai_harness_capture_generation),
            text_staged,
            context: plan
                .expects_context
                .then_some(AgentChatContextStageOutcome {
                    item_kind: "context",
                    staged: context_staged,
                    reason_code: (!context_staged).then_some("context_not_staged"),
                })
                .into_iter()
                .collect(),
            submission,
            blocked_reason: None,
            return_route: plan.return_route,
        })
    }

    fn pending_agent_chat_entry_dispatch(
        &mut self,
        plan: AgentChatEntryCompletionPlan,
        cx: &mut Context<Self>,
    ) -> AgentChatEntryDispatch {
        if let Some(outcome) = self.resolved_agent_chat_entry_outcome(&plan, false, cx) {
            return AgentChatEntryDispatch::Complete(outcome);
        }
        let (tx, rx) = async_channel::bounded(1);
        let request_id = plan.request_id.clone();
        cx.spawn(async move |this, cx| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            loop {
                Timer::after(std::time::Duration::from_millis(25)).await;
                let timed_out = std::time::Instant::now() >= deadline;
                let outcome = this
                    .update(cx, |app, cx| {
                        app.resolved_agent_chat_entry_outcome(&plan, timed_out, cx)
                    })
                    .ok()
                    .flatten();
                if let Some(outcome) = outcome {
                    let _ = tx.send(outcome).await;
                    break;
                }
                if timed_out {
                    let _ = tx
                        .send(AgentChatEntryOutcome {
                            request_id: plan.request_id.clone(),
                            verb: plan.verb,
                            disposition: AgentChatOpenDisposition::Blocked,
                            destination_host: None,
                            destination_thread_id: None,
                            destination_generation: None,
                            text_staged: false,
                            context: Vec::new(),
                            submission: AgentChatSubmissionOutcome::Refused {
                                reason_code: "entry_timeout",
                            },
                            blocked_reason: Some("entry_timeout"),
                            return_route: plan.return_route,
                        })
                        .await;
                    break;
                }
            }
        })
        .detach();
        AgentChatEntryDispatch::Pending(AgentChatEntryTicket {
            request_id,
            completion: rx,
        })
    }

    pub(crate) fn observe_agent_chat_entry_dispatch(
        &mut self,
        dispatch: AgentChatEntryDispatch,
        cx: &mut Context<Self>,
    ) {
        match dispatch {
            AgentChatEntryDispatch::Complete(outcome) => {
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_entry_completed",
                    request_id = %outcome.request_id,
                    verb = ?outcome.verb,
                    feedback = outcome.feedback_verb(),
                    disposition = ?outcome.disposition,
                    submission = ?outcome.submission,
                    outcome_json = %serde_json::to_string(&outcome).unwrap_or_default(),
                );
            }
            AgentChatEntryDispatch::Pending(ticket) => {
                cx.spawn(async move |_this, _cx| {
                    if let Ok(outcome) = ticket.completion.recv().await {
                        tracing::info!(
                            target: "script_kit::tab_ai",
                            event = "agent_chat_entry_completed",
                            request_id = %outcome.request_id,
                            verb = ?outcome.verb,
                            feedback = outcome.feedback_verb(),
                            disposition = ?outcome.disposition,
                            submission = ?outcome.submission,
                            outcome_json = %serde_json::to_string(&outcome).unwrap_or_default(),
                        );
                    }
                })
                .detach();
            }
        }
    }

    pub(crate) fn open_agent_chat_from_entry_request(
        &mut self,
        req: AgentChatEntryRequest,
        cx: &mut Context<Self>,
    ) -> AgentChatEntryDispatch {
        self.dispatch_agent_chat_entry_request(req, cx)
    }

    pub(crate) fn open_and_observe_agent_chat_from_entry_request(
        &mut self,
        req: AgentChatEntryRequest,
        cx: &mut Context<Self>,
    ) {
        let dispatch = self.open_agent_chat_from_entry_request(req, cx);
        self.observe_agent_chat_entry_dispatch(dispatch, cx);
    }

    /// Open the MAIN window's Agent Chat from the Notes window with the
    /// selected note staged as the primary explicit context part and the
    /// persisted note-cart parts staged as supplemental chips.
    ///
    /// Returns `true` only when a live, non-setup main-window Agent Chat
    /// received the primary note context (supplemental staging failures are
    /// logged but do not retroactively fail the handoff).
    pub(crate) fn open_agent_chat_from_notes(
        &mut self,
        target: crate::ai::TabAiTargetContext,
        supplemental_parts: Vec<crate::ai::message_parts::AiContextPart>,
        source: &'static str,
        cx: &mut Context<Self>,
    ) -> bool {
        match self.dispatch_agent_chat_entry_request(
            AgentChatEntryRequest::notes(target, supplemental_parts, source),
            cx,
        ) {
            AgentChatEntryDispatch::Complete(outcome) => outcome.source_consumed(),
            AgentChatEntryDispatch::Pending(ticket) => {
                self.observe_agent_chat_entry_dispatch(AgentChatEntryDispatch::Pending(ticket), cx);
                false
            }
        }
    }

    /// Dispatch an entry request, reporting whether the intended destination
    /// received its initial state/context.
    ///
    /// Legacy (non-Notes) policies keep their historical fire-and-forget
    /// semantics and report `true` unconditionally; only the `NotesHandoff`
    /// branch computes a real staging result, which `open_agent_chat_from_notes`
    /// surfaces to the Notes window for cart-consumption decisions.
    fn dispatch_agent_chat_entry_request(
        &mut self,
        req: AgentChatEntryRequest,
        cx: &mut Context<Self>,
    ) -> AgentChatEntryDispatch {
        if self.agent_chat_surface_state.blocks_launcher_ai_entry() {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "agent_chat_entry_request_blocked_by_portal",
                origin = ?req.origin,
            );
            return AgentChatEntryDispatch::Complete(AgentChatEntryOutcome {
                request_id: req.request_id,
                verb: req.intent.verb(),
                disposition: AgentChatOpenDisposition::Blocked,
                destination_host: None,
                destination_thread_id: None,
                destination_generation: None,
                text_staged: false,
                context: Vec::new(),
                submission: if req.intent.requests_submission() {
                    AgentChatSubmissionOutcome::Refused {
                        reason_code: "portal_active",
                    }
                } else {
                    AgentChatSubmissionOutcome::NotRequested
                },
                blocked_reason: Some("portal_active"),
                return_route: req.return_origin.as_ref().map(|_| "source"),
            });
        }

        let source_view = req
            .return_origin
            .clone()
            .unwrap_or_else(|| self.current_view.clone());
        self.seed_agent_chat_return_origin_for_view(&source_view);

        let entry_verb = req.intent.verb();
        let seed_text = req.intent.seed_text().map(str::to_string);
        let completion_plan = AgentChatEntryCompletionPlan {
            request_id: req.request_id.clone(),
            verb: entry_verb,
            seed_text: seed_text.clone(),
            expects_context: !matches!(
                req.context_policy,
                AgentChatContextPolicy::SuppressFocused | AgentChatContextPolicy::NoContext
            ),
            return_route: req.return_origin.as_ref().map(|_| "source"),
            baseline: self.agent_chat_entry_baseline(cx),
        };
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_entry_request_open",
            origin = ?req.origin,
            target = ?req.target,
            entry_verb = ?entry_verb,
            agent_chat_ui_variant = req.ui_variant.state_id(),
            requests_submission = req.intent.requests_submission(),
            context_policy = ?req.context_policy,
            source_view = ?source_view,
        );

        // Exhaustive on purpose: `CurrentHostEmbedded` means "the main
        // ScriptListApp's own Agent Chat, never the detached chat window" —
        // it is not an alias for `ExistingDetachedOrEmbedded`. Its only
        // producer today is the NotesHandoff policy, whose handler below
        // never consults `chat_window` reuse paths.
        let force_fresh = match req.target {
            AgentChatThreadTarget::ExistingDetachedOrEmbedded => false,
            AgentChatThreadTarget::CurrentHostEmbedded => false,
            AgentChatThreadTarget::FreshEmbedded => true,
        };
        let accepted = match req.context_policy {
            AgentChatContextPolicy::AmbientOrFocused => {
                self.open_tab_ai_agent_chat_with_options(
                    req.intent.clone(),
                    AgentChatContextPolicy::AmbientOrFocused,
                    req.ui_variant,
                    force_fresh,
                    cx,
                );
                true
            }
            AgentChatContextPolicy::SuppressFocused => {
                self.open_tab_ai_agent_chat_with_options(
                    req.intent.clone(),
                    AgentChatContextPolicy::SuppressFocused,
                    req.ui_variant,
                    force_fresh,
                    cx,
                );
                true
            }
            AgentChatContextPolicy::NoContext => {
                self.open_tab_ai_agent_chat_with_options(
                    req.intent.clone(),
                    AgentChatContextPolicy::NoContext,
                    req.ui_variant,
                    force_fresh,
                    cx,
                );
                true
            }
            AgentChatContextPolicy::Parts { parts, source } => {
                if parts.len() != 1 {
                    tracing::error!(
                        target: "script_kit::tab_ai",
                        event = "agent_chat_entry_parts_cardinality_unsupported",
                        part_count = parts.len(),
                    );
                    false
                } else if let Some(part) = parts.into_iter().next() {
                    self.open_tab_ai_agent_chat_with_context_part_and_entry(
                        part,
                        source,
                        req.intent.clone(),
                        cx,
                    );
                    true
                } else {
                    false
                }
            }
            AgentChatContextPolicy::ActionsPayload { target } => {
                self.open_tab_ai_agent_chat_with_explicit_target(target, cx);
                true
            }
            AgentChatContextPolicy::NotesHandoff {
                target,
                supplemental_parts,
                source,
            } => self.open_tab_ai_agent_chat_for_notes_handoff(
                target,
                supplemental_parts,
                source,
                cx,
            ),
            AgentChatContextPolicy::PluginSkill { skill } => {
                self.stage_plugin_skill_from_entry(&skill, cx)
            }
        };
        if !accepted {
            return AgentChatEntryDispatch::Complete(AgentChatEntryOutcome {
                request_id: completion_plan.request_id,
                verb: completion_plan.verb,
                disposition: AgentChatOpenDisposition::Blocked,
                destination_host: None,
                destination_thread_id: None,
                destination_generation: None,
                text_staged: false,
                context: Vec::new(),
                submission: if matches!(
                    completion_plan.verb,
                    AgentChatEntryVerb::Ask | AgentChatEntryVerb::Send
                ) {
                    AgentChatSubmissionOutcome::Refused {
                        reason_code: "entry_refused",
                    }
                } else {
                    AgentChatSubmissionOutcome::NotRequested
                },
                blocked_reason: Some("entry_refused"),
                return_route: completion_plan.return_route,
            });
        }
        self.pending_agent_chat_entry_dispatch(completion_plan, cx)
    }

    /// Main-window handler for the Notes handoff policy.
    ///
    /// Decision order (locked by the Notes handoff contract):
    /// 1. Attachment portal or save-offer overlay active → fail closed.
    /// 2. Current view is a usable full-session Standard Agent Chat (not
    ///    setup, not focused-text mini) → stage into it, preserving messages
    ///    and any composer draft.
    /// 3. Otherwise open a Standard Agent Chat through the main-only
    ///    context-part seam and stage supplemental parts after the view
    ///    exists.
    ///
    /// Never consults the detached chat window; never auto-submits.
    fn open_tab_ai_agent_chat_for_notes_handoff(
        &mut self,
        target: crate::ai::TabAiTargetContext,
        supplemental_parts: Vec<crate::ai::message_parts::AiContextPart>,
        source: &'static str,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.tab_ai_save_offer_state.is_some() {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "notes_handoff_blocked_by_save_offer",
                source,
            );
            return false;
        }

        let label = crate::ai::format_explicit_target_chip_label(&target);
        let semantic_id = target.semantic_id.clone();
        let part = crate::ai::message_parts::AiContextPart::FocusedTarget { target, label };

        // Reuse the live main-window Agent Chat when it is a usable full
        // session: preserve its messages and composer draft.
        if let AppView::AgentChatView { entity, .. } = &self.current_view {
            let entity = entity.clone();
            let reusable = {
                let view = entity.read(cx);
                let is_setup = view.is_setup_mode();
                let is_mini = view.is_focused_text_mini();
                let variant = view.current_ui_variant();
                let policy = view.session_policy();
                let reusable = !is_setup
                    && !is_mini
                    && variant
                        == crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard
                    && policy
                        == crate::ai::agent_chat::ui::capabilities::AgentChatSessionPolicy::Full;
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "notes_handoff_reuse_gate",
                    reusable,
                    is_setup,
                    is_mini,
                    variant = variant.state_id(),
                    policy = ?policy,
                );
                reusable
            };
            if reusable {
                let staged = entity.update(cx, |view, cx| {
                    view.stage_primary_context_part_from_host_preserving_composer(
                        part.clone(),
                        source,
                        cx,
                    )
                });
                return match staged {
                    Ok(()) => {
                        self.stage_notes_supplemental_parts(
                            &entity,
                            supplemental_parts,
                            source,
                            cx,
                        );
                        tracing::info!(
                            target: "script_kit::tab_ai",
                            event = "notes_handoff_main_staged",
                            source,
                            semantic_id = %semantic_id,
                            reused_existing_chat = true,
                        );
                        true
                    }
                    Err(error) => {
                        tracing::warn!(
                            target: "script_kit::tab_ai",
                            event = "notes_handoff_reuse_stage_failed",
                            source,
                            error = %error,
                        );
                        false
                    }
                };
            }
        }

        // Replacing an unsuitable Agent Chat view must not make Agent Chat its
        // own return target: derive a non-chat return origin first.
        let derived_return_origin = match &self.current_view {
            AppView::AgentChatView { .. } => self
                .tab_ai_harness_return_view
                .clone()
                .filter(|view| !matches!(view, AppView::AgentChatView { .. }))
                .unwrap_or(AppView::ScriptList),
            other => other.clone(),
        };

        self.open_tab_ai_agent_chat_with_context_part(part, source, cx);

        let staged_entity = match &self.current_view {
            AppView::AgentChatView { entity, .. } if !entity.read(cx).is_setup_mode() => {
                Some(entity.clone())
            }
            _ => None,
        };
        let Some(entity) = staged_entity else {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "notes_handoff_main_stage_failed",
                source,
                semantic_id = %semantic_id,
                reason = "no_live_agent_chat_view_after_open",
            );
            return false;
        };

        self.tab_ai_harness_return_view = Some(derived_return_origin);
        self.stage_notes_supplemental_parts(&entity, supplemental_parts, source, cx);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "notes_handoff_main_staged",
            source,
            semantic_id = %semantic_id,
            reused_existing_chat = false,
        );
        true
    }

    /// Stage persisted note-cart parts as supplemental context chips without
    /// touching the composer. A supplemental failure is logged, never fatal —
    /// the primary note context already staged successfully.
    fn stage_notes_supplemental_parts(
        &mut self,
        entity: &gpui::Entity<crate::ai::agent_chat::ui::AgentChatView>,
        supplemental_parts: Vec<crate::ai::message_parts::AiContextPart>,
        source: &'static str,
        cx: &mut Context<Self>,
    ) {
        if supplemental_parts.is_empty() {
            return;
        }
        let result = entity.update(cx, |view, cx| {
            view.stage_supplemental_context_parts_from_host(supplemental_parts, source, cx)
        });
        if let Err(error) = result {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "notes_handoff_supplemental_stage_failed",
                source,
                error = %error,
            );
        }
    }
}

#[cfg(test)]
mod quick_question_contract {
    // Deliberately no `use super::*`: the parent glob-imports gpui, whose
    // `test` macro would shadow the builtin `#[test]` attribute.
    use super::{
        AgentChatContextPolicy, AgentChatEntryIntent, AgentChatEntryOutcome, AgentChatEntryRequest,
        AgentChatEntryVerb, AgentChatOpenDisposition, AgentChatSubmissionOutcome,
    };

    /// Double-tap of the main hotkey means "fastest path to a clean chat for
    /// a quick question". It must never carry the launcher's auto-selected
    /// row (or any other implicit context) into Agent Chat.
    #[test]
    fn quick_question_entry_suppresses_all_implicit_context() {
        let req = AgentChatEntryRequest::quick_question();
        assert_eq!(
            req.context_policy,
            AgentChatContextPolicy::NoContext,
            "quick-question entry must suppress every implicit context source",
        );
        assert_eq!(req.intent, AgentChatEntryIntent::Open { seed_text: None });
        assert_eq!(req.intent.verb(), AgentChatEntryVerb::Open);
        assert!(
            !req.intent.requests_submission(),
            "quick-question entry must never auto-submit",
        );
    }

    #[test]
    fn quick_ai_handoff_is_fresh_composer_only_and_context_free() {
        let req = AgentChatEntryRequest::quick_ai_handoff("bounded result".to_string());
        assert_eq!(req.target, super::AgentChatThreadTarget::FreshEmbedded,);
        assert_eq!(
            req.intent,
            AgentChatEntryIntent::Continue {
                seed_text: Some("bounded result".to_string())
            }
        );
        assert!(!req.intent.requests_submission());
        assert_eq!(req.context_policy, AgentChatContextPolicy::SuppressFocused,);
    }

    #[test]
    fn launcher_open_with_text_stages_without_submitting() {
        let req = AgentChatEntryRequest::main_launcher(Some("draft question".to_string()));
        assert_eq!(req.intent.verb(), AgentChatEntryVerb::Open);
        assert_eq!(req.intent.seed_text(), Some("draft question"));
        assert!(!req.intent.requests_submission());
    }

    #[test]
    fn quick_ai_with_a_real_query_constructs_exactly_one_ask() {
        use crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant;
        let req = AgentChatEntryRequest::main_launcher_with_variant(
            Some("answer this".to_string()),
            AgentChatUiVariant::QuickAi,
        );
        assert_eq!(req.intent.verb(), AgentChatEntryVerb::Ask);
        assert_eq!(req.intent.seed_text(), Some("answer this"));
        assert!(req.intent.requests_submission());
        assert_eq!(req.context_policy, AgentChatContextPolicy::NoContext);
    }

    #[test]
    fn ask_and_send_reject_empty_explicit_submissions() {
        assert_eq!(
            AgentChatEntryIntent::ask("  ".to_string()),
            Err("ask_requires_nonempty_text")
        );
        assert_eq!(
            AgentChatEntryIntent::send("\n".to_string()),
            Err("send_requires_nonempty_text")
        );
    }

    #[test]
    fn entry_outcome_serialization_is_redacted_and_feedback_is_truthful() {
        let outcome = AgentChatEntryOutcome {
            request_id: "agent-chat-entry-test".to_string(),
            verb: AgentChatEntryVerb::Ask,
            disposition: AgentChatOpenDisposition::OpenedFresh,
            destination_host: Some("main".to_string()),
            destination_thread_id: Some("test-thread".to_string()),
            destination_generation: Some(7),
            text_staged: true,
            context: Vec::new(),
            submission: AgentChatSubmissionOutcome::Accepted,
            blocked_reason: None,
            return_route: Some("source"),
        };
        assert_eq!(outcome.feedback_verb(), "Asked");
        assert!(outcome.source_consumed());
        let json = serde_json::to_string(&outcome).unwrap_or_default();
        assert!(json.contains("agent-chat-entry-test"));
        assert!(!json.contains("rawText"));
        assert!(!json.contains("seedText"));
        assert!(!json.contains("prompt"));
    }

    #[test]
    fn pending_or_refused_submission_never_claims_asked_or_sent() {
        for (verb, submission) in [
            (AgentChatEntryVerb::Ask, AgentChatSubmissionOutcome::Pending),
            (
                AgentChatEntryVerb::Send,
                AgentChatSubmissionOutcome::Refused {
                    reason_code: "runtime_refused",
                },
            ),
        ] {
            let outcome = AgentChatEntryOutcome {
                request_id: "agent-chat-entry-test".to_string(),
                verb,
                disposition: AgentChatOpenDisposition::Reused,
                destination_host: Some("main".to_string()),
                destination_thread_id: Some("test-thread".to_string()),
                destination_generation: Some(8),
                text_staged: false,
                context: Vec::new(),
                submission,
                blocked_reason: None,
                return_route: None,
            };
            assert_eq!(outcome.feedback_verb(), "Refused");
            assert!(!outcome.source_consumed());
        }
    }

    fn notes_target_fixture() -> crate::ai::TabAiTargetContext {
        crate::ai::TabAiTargetContext {
            source: "Notes".to_string(),
            kind: "note".to_string(),
            semantic_id: "note:test-fixture".to_string(),
            label: "Test Note".to_string(),
            metadata: None,
        }
    }

    /// The Notes handoff is main-host-only, composer-only, and fully
    /// explicit: origin Notes, target CurrentHostEmbedded (never the
    /// detached chat window), Standard variant, no seed text, no
    /// auto-submit, and a NotesHandoff context policy.
    #[test]
    fn notes_entry_is_main_host_composer_only_and_explicit() {
        let req = AgentChatEntryRequest::notes(notes_target_fixture(), Vec::new(), "test");
        assert_eq!(
            req.target,
            super::AgentChatThreadTarget::CurrentHostEmbedded,
            "notes entry must target the main host's own Agent Chat",
        );
        assert_ne!(
            req.target,
            super::AgentChatThreadTarget::ExistingDetachedOrEmbedded,
            "notes entry must never target the detached chat window",
        );
        assert_eq!(req.intent, AgentChatEntryIntent::Add { seed_text: None });
        assert!(
            !req.intent.requests_submission(),
            "notes entry must never auto-submit",
        );
        assert!(
            matches!(
                req.context_policy,
                AgentChatContextPolicy::NotesHandoff { .. }
            ),
            "notes entry must carry the NotesHandoff context policy",
        );
    }

    /// The launcher's implicitly selected row must never leak into a
    /// Notes-origin request.
    #[test]
    fn notes_entry_never_admits_implicit_focused_context() {
        let req = AgentChatEntryRequest::notes(notes_target_fixture(), Vec::new(), "test");
        assert!(
            !req.context_policy.admits_implicit_focused_part(),
            "NotesHandoff must suppress the implicit focused row",
        );
    }

    /// Behavior replacement for the former source-string audit that lived in
    /// `src/ai/agent_chat/ui/tests.rs`: Standard launcher entry may inherit the
    /// selected launcher row, every nonstandard variant must suppress it. Kept
    /// under the same test name so existing CI filters continue to work.
    #[test]
    fn agent_chat_ui_variant_launch_suppresses_selected_launcher_row_context_contract() {
        use crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant;
        let standard =
            AgentChatEntryRequest::main_launcher_with_variant(None, AgentChatUiVariant::Standard);
        assert_eq!(
            standard.context_policy,
            AgentChatContextPolicy::AmbientOrFocused,
            "Standard launcher entry may inherit the selected launcher row",
        );
        for variant in [
            AgentChatUiVariant::UserBold,
            AgentChatUiVariant::RoleSplit,
            AgentChatUiVariant::BottomDock,
            AgentChatUiVariant::DenseLog,
            AgentChatUiVariant::Sidecar,
            AgentChatUiVariant::FocusedTextMini,
        ] {
            let req = AgentChatEntryRequest::main_launcher_with_variant(None, variant);
            assert_eq!(
                req.context_policy,
                AgentChatContextPolicy::SuppressFocused,
                "{variant:?} launcher entry must not inherit the selected launcher row",
            );
        }
        let quick_ai = AgentChatEntryRequest::main_launcher_with_variant(
            None,
            AgentChatUiVariant::QuickAi,
        );
        assert_eq!(
            quick_ai.context_policy,
            AgentChatContextPolicy::NoContext,
            "Quick AI must suppress every context source",
        );
    }
}
