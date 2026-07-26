//! One canonical backgrounded-AI-conversation store (spec §8 step 6,
//! `docs/specs/backgrounded-ai-sessions.md`, Oracle plan
//! `.notes/oracle/color-consistency-escape/oracle-output.log`).
//!
//! Every surface that can background a resumable AI session — Flow, Agent
//! Chat, Quick AI — is enumerated by exactly ONE owner:
//! [`BackgroundedSessionStore`], a field on app state. Surface-specific
//! caches (warm leases, prewarmed entities) may keep provider resources
//! alive, but they must never independently claim which sessions are
//! backgrounded or produce separate main-menu rows.
//!
//! Atomicity is structural, not procedural: metadata and its GPUI entity
//! handle live in the SAME container element (a pair), so their counts
//! cannot diverge and removal drops both or neither. This is deliberately
//! stronger than the "keep two maps in sync" shape — the failure mode the
//! plan warns about ("entity and metadata counts diverge") is
//! unrepresentable here.
//!
//! Lifecycle contract: the store lives OUTSIDE `current_view`. It survives
//! `reset_to_script_list`, `close_and_reset_window`, and filtering-cache
//! rebuilds. Only explicit session close/removal (or app teardown) shrinks
//! it. Persistence and expiry remain unauthorized (spec G2).

use std::time::SystemTime;

// ---------------------------------------------------------------------------
// Tagged session identity
// ---------------------------------------------------------------------------

/// Flow conversational session id (`FlowSessionMeta::id`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FlowSessionId(pub u64);

/// Agent Chat session identity (semantic id string).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AgentChatSessionId(pub String);

/// Quick AI session identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QuickAiSessionId(pub u64);

/// Collision-proof, surface-tagged session key.
///
/// Deliberately NOT an untagged integer shared across surfaces: Flow ids and
/// Quick AI ids both count from small integers, and an untagged key would let
/// `flow:1` and `quick-ai:1` collide the first time both exist. The tag also
/// gives the recency sort a deterministic total order for timestamp ties.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ConversationSessionId {
    AgentChat(AgentChatSessionId),
    Flow(FlowSessionId),
    QuickAi(QuickAiSessionId),
}

impl ConversationSessionId {
    /// Stable, privacy-safe wire form for driver receipts (`flow:17`).
    pub fn automation_id(&self) -> String {
        match self {
            Self::AgentChat(AgentChatSessionId(id)) => format!("agent-chat:{id}"),
            Self::Flow(FlowSessionId(id)) => format!("flow:{id}"),
            Self::QuickAi(QuickAiSessionId(id)) => format!("quick-ai:{id}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Record model
// ---------------------------------------------------------------------------

/// Which product surface owns the conversation. Kept separate from dock
/// presentation state — surface kind is product identity, not UI state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConversationSurface {
    AgentChat { profile_id: String },
    Flow { flow_id: String },
    QuickAi,
}

impl ConversationSurface {
    /// Privacy-safe wire label for driver receipts.
    pub fn automation_label(&self) -> &'static str {
        match self {
            Self::AgentChat { .. } => "agentChat",
            Self::Flow { .. } => "flow",
            Self::QuickAi => "quickAi",
        }
    }
}

/// Conversation liveness, independent of any one surface's internal state
/// machine. `Failed` sessions remain resumable rows; a user Stop is
/// cancellation and must map to `Idle`, never `Failed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConversationLiveness {
    Live { turn_in_flight: bool },
    Idle,
    Failed { code: String },
}

impl ConversationLiveness {
    pub fn turn_in_flight(&self) -> bool {
        matches!(
            self,
            Self::Live {
                turn_in_flight: true
            }
        )
    }

    /// Privacy-safe wire label for driver receipts.
    pub fn automation_label(&self) -> &'static str {
        match self {
            Self::Live { .. } => "live",
            Self::Idle => "idle",
            Self::Failed { .. } => "failed",
        }
    }
}

/// Presentation metadata for one backgrounded conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationRecord {
    pub id: ConversationSessionId,
    pub surface: ConversationSurface,
    /// Recognizable one-line title (never rendered as "Untitled session"
    /// three times — the projecting surface owns a real fallback ladder).
    pub title: String,
    /// Surface/engine/profile context line.
    pub subtitle: String,
    /// Last SEMANTIC activity: creation, explicit resume/open, turn submit,
    /// or a turn reaching a terminal state. Never per streamed token.
    pub last_activity: SystemTime,
    pub liveness: ConversationLiveness,
}

/// Order canonical for every enumeration of backgrounded conversations:
/// `last_activity` descending, then tagged stable id descending. The id
/// tie-break keeps equal timestamps from reordering rows frame-to-frame
/// (keyboard selection must never watch two rows swap under it).
pub fn sort_conversation_records_by_recency(records: &mut [ConversationRecord]) {
    records.sort_by(|a, b| {
        b.last_activity
            .cmp(&a.last_activity)
            .then_with(|| b.id.cmp(&a.id))
    });
}

// ---------------------------------------------------------------------------
// Pure ledger: record + entity pairs, atomic by construction
// ---------------------------------------------------------------------------

/// Typed outcomes so callers cannot mistake "session unknown" for success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationMutation {
    Applied,
    MissingSession,
}

/// Pure pairing of [`ConversationRecord`]s with their retained entities.
///
/// Generic over the entity handle so the model is testable without GPUI:
/// production uses [`SessionEntity`], tests use a plain label enum. All
/// mutations go through methods — the pair container is private, so a
/// record can never exist without its entity or vice versa.
#[derive(Debug, Default)]
pub struct ConversationLedger<E> {
    entries: Vec<(ConversationRecord, E)>,
}

impl<E> ConversationLedger<E> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Insert a session or atomically replace BOTH the record and entity of
    /// an existing one. Duplicate ids never produce duplicate rows.
    pub fn insert_or_replace(&mut self, record: ConversationRecord, entity: E) {
        match self
            .entries
            .iter_mut()
            .find(|(existing, _)| existing.id == record.id)
        {
            Some(slot) => *slot = (record, entity),
            None => self.entries.push((record, entity)),
        }
    }

    /// Mark semantic activity (creation, resume, submit, terminal state).
    pub fn touch(&mut self, id: &ConversationSessionId, at: SystemTime) -> ConversationMutation {
        match self.record_mut(id) {
            Some(record) => {
                record.last_activity = at;
                ConversationMutation::Applied
            }
            None => ConversationMutation::MissingSession,
        }
    }

    /// Liveness transitions are semantic activity, so they also touch.
    pub fn set_liveness(
        &mut self,
        id: &ConversationSessionId,
        liveness: ConversationLiveness,
        at: SystemTime,
    ) -> ConversationMutation {
        match self.record_mut(id) {
            Some(record) => {
                record.liveness = liveness;
                record.last_activity = at;
                ConversationMutation::Applied
            }
            None => ConversationMutation::MissingSession,
        }
    }

    pub fn get(&self, id: &ConversationSessionId) -> Option<&ConversationRecord> {
        self.entries
            .iter()
            .find(|(record, _)| &record.id == id)
            .map(|(record, _)| record)
    }

    pub fn entity(&self, id: &ConversationSessionId) -> Option<&E> {
        self.entries
            .iter()
            .find(|(record, _)| &record.id == id)
            .map(|(_, entity)| entity)
    }

    /// Remove a session, returning the record AND entity together — the
    /// caller provably gets both or neither.
    pub fn remove(&mut self, id: &ConversationSessionId) -> Option<(ConversationRecord, E)> {
        let index = self
            .entries
            .iter()
            .position(|(record, _)| &record.id == id)?;
        Some(self.entries.remove(index))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn records(&self) -> impl Iterator<Item = &ConversationRecord> {
        self.entries.iter().map(|(record, _)| record)
    }

    fn record_mut(&mut self, id: &ConversationSessionId) -> Option<&mut ConversationRecord> {
        self.entries
            .iter_mut()
            .find(|(record, _)| &record.id == id)
            .map(|(record, _)| record)
    }
}

// ---------------------------------------------------------------------------
// App-state store (GPUI entity handles)
// ---------------------------------------------------------------------------

/// Retained GPUI handle for one backgrounded conversation, tagged by the
/// surface that knows how to resume it.
#[derive(Clone)]
pub(crate) enum SessionEntity {
    AgentChat(gpui::Entity<crate::ai::agent_chat::ui::view::AgentChatView>),
    Flow(gpui::Entity<crate::prompts::ChatPrompt>),
    QuickAi(gpui::Entity<crate::ai::agent_chat::ui::view::AgentChatView>),
}

impl SessionEntity {
    /// Whether this handle's kind matches the id's surface tag.
    fn matches_id(&self, id: &ConversationSessionId) -> bool {
        matches!(
            (self, id),
            (Self::AgentChat(_), ConversationSessionId::AgentChat(_))
                | (Self::Flow(_), ConversationSessionId::Flow(_))
                | (Self::QuickAi(_), ConversationSessionId::QuickAi(_))
        )
    }
}

/// The one canonical enumerator of resumable/backgrounded AI sessions.
///
/// Flow sessions keep their rich [`FlowSessionMeta`] as the single metadata
/// authority — their [`ConversationRecord`] is a PROJECTION computed on
/// read ([`flow_conversation_record`]), never a stored copy that could
/// drift. Agent Chat and Quick AI records live in the pure ledger. Both
/// containers pair metadata with entity, so neither can lose one side.
///
/// `flow_sessions` stays a `pub(crate)` field (not a method accessor) so
/// the ~95 existing call sites keep disjoint-field borrow splitting; the
/// pair layout preserves the atomicity contract those sites rely on. New
/// enumeration MUST go through [`Self::ordered_rows`] — never iterate a
/// surface's storage to build a second session list.
pub(crate) struct BackgroundedSessionStore {
    /// Live conversational flow sessions (Enter = converse). Each pairs
    /// metadata with its Threadline ChatPrompt entity; backgrounding keeps
    /// the entity (and any in-flight turn) alive, re-entering an Active row
    /// restores the SAME transcript.
    pub(crate) flow_sessions: Vec<(
        crate::flows::session::FlowSessionMeta,
        gpui::Entity<crate::prompts::ChatPrompt>,
    )>,
    /// Monotonic id source for flow sessions.
    pub(crate) flow_session_counter: u64,
    /// Agent Chat / Quick AI records + entities. Production insertion only
    /// where the app already retains a per-session conversation (today:
    /// none — the warm/prewarmed Agent Chat entities are reuse caches, not
    /// backgrounded sessions, and Quick AI must not resurrect from closed
    /// history). The model, ordering, resume dispatch, and renderer support
    /// them so step 7 can render mixed fixtures and later steps can wire
    /// production records without reshaping the store.
    detached: ConversationLedger<SessionEntity>,
}

impl BackgroundedSessionStore {
    pub(crate) fn new() -> Self {
        Self {
            flow_sessions: Vec::new(),
            flow_session_counter: 0,
            detached: ConversationLedger::new(),
        }
    }

    /// Insert or atomically replace a detached (Agent Chat / Quick AI)
    /// session. Flow sessions are owned by `flow_sessions`; inserting one
    /// here would create the "two enumerators for one session" defect, so
    /// it is rejected as a typed outcome.
    pub(crate) fn insert_or_replace(
        &mut self,
        record: ConversationRecord,
        entity: SessionEntity,
    ) -> ConversationMutation {
        if matches!(record.id, ConversationSessionId::Flow(_)) || !entity.matches_id(&record.id) {
            return ConversationMutation::MissingSession;
        }
        self.detached.insert_or_replace(record, entity);
        ConversationMutation::Applied
    }

    /// Mark semantic activity on any surface's session.
    pub(crate) fn touch(
        &mut self,
        id: &ConversationSessionId,
        at: SystemTime,
    ) -> ConversationMutation {
        match id {
            ConversationSessionId::Flow(FlowSessionId(flow_id)) => {
                match self
                    .flow_sessions
                    .iter_mut()
                    .find(|(meta, _)| meta.id == *flow_id)
                {
                    Some((meta, _)) => {
                        meta.touch_at(at);
                        ConversationMutation::Applied
                    }
                    None => ConversationMutation::MissingSession,
                }
            }
            _ => self.detached.touch(id, at),
        }
    }

    /// Set liveness on a detached session. Flow liveness DERIVES from the
    /// flow state machine (`SessionState` + `active_turn` + reliability);
    /// writing it externally would let the projection and the machine
    /// disagree, so Flow ids are rejected as a typed outcome.
    pub(crate) fn set_liveness(
        &mut self,
        id: &ConversationSessionId,
        liveness: ConversationLiveness,
        at: SystemTime,
    ) -> ConversationMutation {
        match id {
            ConversationSessionId::Flow(_) => ConversationMutation::MissingSession,
            _ => self.detached.set_liveness(id, liveness, at),
        }
    }

    /// The projected record for any surface's session.
    pub(crate) fn get(&self, id: &ConversationSessionId) -> Option<ConversationRecord> {
        match id {
            ConversationSessionId::Flow(FlowSessionId(flow_id)) => self
                .flow_sessions
                .iter()
                .find(|(meta, _)| meta.id == *flow_id)
                .map(|(meta, _)| flow_conversation_record(meta)),
            _ => self.detached.get(id).cloned(),
        }
    }

    /// Remove a session — metadata and entity together, atomically.
    pub(crate) fn remove(&mut self, id: &ConversationSessionId) -> ConversationMutation {
        match id {
            ConversationSessionId::Flow(FlowSessionId(flow_id)) => {
                let before = self.flow_sessions.len();
                self.flow_sessions.retain(|(meta, _)| meta.id != *flow_id);
                if self.flow_sessions.len() < before {
                    ConversationMutation::Applied
                } else {
                    ConversationMutation::MissingSession
                }
            }
            _ => match self.detached.remove(id) {
                Some(_) => ConversationMutation::Applied,
                None => ConversationMutation::MissingSession,
            },
        }
    }

    /// The retained entity to resume, tagged with its surface kind.
    pub(crate) fn resume_entity(&self, id: &ConversationSessionId) -> Option<SessionEntity> {
        match id {
            ConversationSessionId::Flow(FlowSessionId(flow_id)) => self
                .flow_sessions
                .iter()
                .find(|(meta, _)| meta.id == *flow_id)
                .map(|(_, entity)| SessionEntity::Flow(entity.clone())),
            _ => self.detached.entity(id).cloned(),
        }
    }

    /// Every backgrounded session across every surface, canonically ordered
    /// (`last_activity` desc, tagged id desc). THE enumeration for any
    /// "Conversations" style listing.
    pub(crate) fn ordered_rows(&self) -> Vec<ConversationRecord> {
        let mut rows: Vec<ConversationRecord> = self
            .flow_sessions
            .iter()
            .map(|(meta, _)| flow_conversation_record(meta))
            .chain(self.detached.records().cloned())
            .collect();
        sort_conversation_records_by_recency(&mut rows);
        rows
    }

    pub(crate) fn len(&self) -> usize {
        self.flow_sessions.len() + self.detached.len()
    }

    /// Privacy-safe driver receipt: counts, tagged ids, surface, liveness,
    /// and activity clocks only. No transcripts, prompts, drafts, provider
    /// payloads, or titles.
    pub(crate) fn snapshot(&self) -> serde_json::Value {
        let sessions: Vec<serde_json::Value> = self
            .ordered_rows()
            .iter()
            .map(|record| {
                serde_json::json!({
                    "id": record.id.automation_id(),
                    "surface": record.surface.automation_label(),
                    "liveness": record.liveness.automation_label(),
                    "turnInFlight": record.liveness.turn_in_flight(),
                    "lastActivityUnixMs": record
                        .last_activity
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|elapsed| elapsed.as_millis() as u64)
                        .unwrap_or(0),
                })
            })
            .collect();
        serde_json::json!({
            "count": self.len(),
            "sessions": sessions,
        })
    }
}

/// Project a Flow session's rich metadata into the shared conversation
/// record. Computed on read so the flow state machine remains the single
/// authority — there is no stored copy to drift.
pub(crate) fn flow_conversation_record(
    meta: &crate::flows::session::FlowSessionMeta,
) -> ConversationRecord {
    use sk_protocol::ai_reliability::AiPhase;

    let liveness =
        if let AiPhase::AwaitingRecovery { failure, .. } = &meta.reliability.state().phase {
            ConversationLiveness::Failed {
                code: format!("{:?}", failure.code),
            }
        } else if meta.active_turn.is_some() {
            ConversationLiveness::Live {
                turn_in_flight: true,
            }
        } else if matches!(meta.state, crate::flows::session::SessionState::Working) {
            ConversationLiveness::Live {
                turn_in_flight: false,
            }
        } else {
            ConversationLiveness::Idle
        };

    ConversationRecord {
        id: ConversationSessionId::Flow(FlowSessionId(meta.id)),
        surface: ConversationSurface::Flow {
            flow_id: meta.flow_id.clone(),
        },
        title: meta.friendly_name.clone(),
        subtitle: format!("Flow · {}", meta.engine),
        last_activity: meta.last_activity,
        liveness,
    }
}

#[cfg(test)]
mod conversation_model_tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    /// Test double for the entity side: the kind label is enough to prove
    /// routing/atomicity without GPUI.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum TestEntity {
        AgentChat,
        Flow,
        QuickAi,
    }

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn record(id: ConversationSessionId, seconds: u64) -> ConversationRecord {
        let surface = match &id {
            ConversationSessionId::AgentChat(_) => ConversationSurface::AgentChat {
                profile_id: "default".into(),
            },
            ConversationSessionId::Flow(_) => ConversationSurface::Flow {
                flow_id: "project:test".into(),
            },
            ConversationSessionId::QuickAi(_) => ConversationSurface::QuickAi,
        };
        ConversationRecord {
            id,
            surface,
            title: "Fix the footer color drift".into(),
            subtitle: "Agent Chat · GPT-5.6".into(),
            last_activity: at(seconds),
            liveness: ConversationLiveness::Idle,
        }
    }

    fn flow_id(id: u64) -> ConversationSessionId {
        ConversationSessionId::Flow(FlowSessionId(id))
    }

    fn agent_chat_id(id: &str) -> ConversationSessionId {
        ConversationSessionId::AgentChat(AgentChatSessionId(id.into()))
    }

    fn quick_ai_id(id: u64) -> ConversationSessionId {
        ConversationSessionId::QuickAi(QuickAiSessionId(id))
    }

    /// The reason the key is tagged: Flow and Quick AI both count sessions
    /// from small integers, so an untagged shared integer would collide the
    /// first time both surfaces hold session 7.
    #[test]
    fn all_three_surface_kinds_coexist_without_id_collision() {
        let mut ledger = ConversationLedger::new();
        ledger.insert_or_replace(record(flow_id(7), 1), TestEntity::Flow);
        ledger.insert_or_replace(record(quick_ai_id(7), 2), TestEntity::QuickAi);
        ledger.insert_or_replace(record(agent_chat_id("7"), 3), TestEntity::AgentChat);
        assert_eq!(ledger.len(), 3, "same inner value, three distinct sessions");
        assert!(ledger.get(&flow_id(7)).is_some());
        assert!(ledger.get(&quick_ai_id(7)).is_some());
        assert!(ledger.get(&agent_chat_id("7")).is_some());
    }

    #[test]
    fn duplicate_insertion_replaces_rather_than_duplicates() {
        let mut ledger = ConversationLedger::new();
        ledger.insert_or_replace(record(flow_id(1), 10), TestEntity::Flow);
        let mut renamed = record(flow_id(1), 20);
        renamed.title = "Renamed".into();
        ledger.insert_or_replace(renamed, TestEntity::Flow);
        assert_eq!(ledger.len(), 1);
        assert_eq!(ledger.get(&flow_id(1)).expect("record").title, "Renamed");
    }

    #[test]
    fn touch_moves_a_session_to_the_front_of_the_recency_order() {
        let mut ledger = ConversationLedger::new();
        ledger.insert_or_replace(record(flow_id(1), 100), TestEntity::Flow);
        ledger.insert_or_replace(record(flow_id(2), 200), TestEntity::Flow);
        assert_eq!(
            ledger.touch(&flow_id(1), at(300)),
            ConversationMutation::Applied
        );
        let mut rows: Vec<ConversationRecord> = ledger.records().cloned().collect();
        sort_conversation_records_by_recency(&mut rows);
        assert_eq!(rows[0].id, flow_id(1), "touched older session leads");
    }

    /// `None`-shaped outcomes must be typed: touching an unknown session is
    /// a `MissingSession`, never a silent no-op the caller reads as success.
    #[test]
    fn mutating_an_unknown_session_reports_missing() {
        let mut ledger: ConversationLedger<TestEntity> = ConversationLedger::new();
        assert_eq!(
            ledger.touch(&flow_id(9), at(1)),
            ConversationMutation::MissingSession
        );
        assert_eq!(
            ledger.set_liveness(&flow_id(9), ConversationLiveness::Idle, at(1)),
            ConversationMutation::MissingSession
        );
    }

    /// Removal returns the record AND entity in one value — the caller
    /// provably holds both, so a dead row can never linger with a live
    /// entity (or vice versa).
    #[test]
    fn removal_drops_metadata_and_entity_atomically() {
        let mut ledger = ConversationLedger::new();
        ledger.insert_or_replace(record(agent_chat_id("a"), 1), TestEntity::AgentChat);
        let (removed_record, removed_entity) =
            ledger.remove(&agent_chat_id("a")).expect("removed pair");
        assert_eq!(removed_record.id, agent_chat_id("a"));
        assert_eq!(removed_entity, TestEntity::AgentChat);
        assert!(ledger.get(&agent_chat_id("a")).is_none());
        assert!(ledger.entity(&agent_chat_id("a")).is_none());
        assert!(ledger.is_empty());
    }

    /// A failed turn leaves a RESUMABLE session: the row stays, marked
    /// failed, and its liveness change counts as semantic activity.
    #[test]
    fn failed_resumable_session_remains_until_explicitly_closed() {
        let mut ledger = ConversationLedger::new();
        ledger.insert_or_replace(record(flow_id(1), 100), TestEntity::Flow);
        assert_eq!(
            ledger.set_liveness(
                &flow_id(1),
                ConversationLiveness::Failed {
                    code: "ProviderOverloaded".into()
                },
                at(150),
            ),
            ConversationMutation::Applied
        );
        let failed = ledger.get(&flow_id(1)).expect("failed row remains");
        assert_eq!(
            failed.liveness,
            ConversationLiveness::Failed {
                code: "ProviderOverloaded".into()
            }
        );
        assert_eq!(failed.last_activity, at(150), "terminal transition touches");

        // Explicit close is the only removal path.
        assert!(ledger.remove(&flow_id(1)).is_some());
        assert!(ledger.get(&flow_id(1)).is_none());
    }

    /// Equal timestamps must not reorder frame-to-frame: the tagged id
    /// descending tie-break gives a deterministic total order.
    #[test]
    fn equal_timestamps_order_by_stable_id_descending() {
        let mut rows = vec![
            record(flow_id(1), 100),
            record(flow_id(3), 100),
            record(flow_id(2), 100),
        ];
        sort_conversation_records_by_recency(&mut rows);
        let ids: Vec<&ConversationSessionId> = rows.iter().map(|row| &row.id).collect();
        assert_eq!(ids, vec![&flow_id(3), &flow_id(2), &flow_id(1)]);
    }
}
