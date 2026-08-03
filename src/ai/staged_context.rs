//! Typed lifecycle for Agent Chat context staged ahead of a turn.
//!
//! `AiContextPart` owns the model-bound source. This module owns the UI/runtime
//! facts around that source: how it arrived, whether it is required, which
//! lifecycle state it is in, and whether the user may remove it.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::ai::message_parts::{AiContextPart, ContextPreparationRole, ContextSourceKind};
use crate::ai::reliability::AppFailureRecord;

static NEXT_CONTEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContextItemId(pub String);

impl ContextItemId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContextProvenance {
    ImplicitFocused,
    DeferredAmbient,
    UserMention,
    AttachmentPortal,
    HostHandoff,
    ThreadReceipt,
}

impl ContextProvenance {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ImplicitFocused => "implicitFocused",
            Self::DeferredAmbient => "deferredAmbient",
            Self::UserMention => "userMention",
            Self::AttachmentPortal => "attachmentPortal",
            Self::HostHandoff => "hostHandoff",
            Self::ThreadReceipt => "threadReceipt",
        }
    }

    fn pending_priority(self) -> u8 {
        match self {
            Self::DeferredAmbient => 1,
            Self::ImplicitFocused => 2,
            Self::HostHandoff => 3,
            Self::AttachmentPortal => 4,
            Self::UserMention => 5,
            Self::ThreadReceipt => 0,
        }
    }

    pub fn cue(self) -> &'static str {
        match self {
            Self::ImplicitFocused => "Suggested",
            Self::DeferredAmbient => "Captures on send",
            Self::UserMention | Self::AttachmentPortal => "Added",
            Self::HostHandoff => "Added from another surface",
            Self::ThreadReceipt => "Used in this turn",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContextRole {
    Primary,
    Supplemental,
}

impl ContextRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Supplemental => "supplemental",
        }
    }

    pub fn preparation_role(self) -> ContextPreparationRole {
        match self {
            Self::Primary => ContextPreparationRole::Primary,
            Self::Supplemental => ContextPreparationRole::Supplemental,
        }
    }

    fn priority(self) -> u8 {
        match self {
            Self::Primary => 2,
            Self::Supplemental => 1,
        }
    }
}

impl From<ContextPreparationRole> for ContextRole {
    fn from(value: ContextPreparationRole) -> Self {
        match value {
            ContextPreparationRole::Primary => Self::Primary,
            ContextPreparationRole::Supplemental => Self::Supplemental,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextLifecycleState {
    Pending,
    Resolving,
    Resolved,
    Failed { failure: AppFailureRecord },
}

impl ContextLifecycleState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Resolving => "resolving",
            Self::Resolved => "resolved",
            Self::Failed { .. } => "failed",
        }
    }

    pub fn failure(&self) -> Option<&AppFailureRecord> {
        match self {
            Self::Failed { failure } => Some(failure),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContextLifetime {
    NextTurn,
    ImmutableReceipt,
}

impl ContextLifetime {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NextTurn => "nextTurn",
            Self::ImmutableReceipt => "immutableReceipt",
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct StagedContextItem {
    pub id: ContextItemId,
    pub part: AiContextPart,
    pub provenance: ContextProvenance,
    pub role: ContextRole,
    pub state: ContextLifecycleState,
    pub lifetime: ContextLifetime,
    pub removable: bool,
    pub generation: u64,
}

impl std::fmt::Debug for StagedContextItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StagedContextItem")
            .field("id", &self.id)
            .field("source_kind", &self.part.source_kind())
            .field("provenance", &self.provenance)
            .field("role", &self.role)
            .field("state", &self.state.as_str())
            .field("lifetime", &self.lifetime)
            .field("removable", &self.removable)
            .field("generation", &self.generation)
            .finish()
    }
}

impl StagedContextItem {
    pub fn pending(part: AiContextPart, provenance: ContextProvenance, role: ContextRole) -> Self {
        let generation = NEXT_CONTEXT_GENERATION.fetch_add(1, Ordering::Relaxed);
        let identity_hash = context_identity_hash(&part);
        Self {
            id: ContextItemId(format!("context-{identity_hash:016x}-{generation}")),
            part,
            provenance,
            role,
            state: ContextLifecycleState::Pending,
            lifetime: ContextLifetime::NextTurn,
            removable: true,
            generation,
        }
    }

    pub fn immutable_receipt_from(mut self) -> Self {
        self.provenance = ContextProvenance::ThreadReceipt;
        self.state = ContextLifecycleState::Resolved;
        self.lifetime = ContextLifetime::ImmutableReceipt;
        self.removable = false;
        self
    }

    pub fn source_kind(&self) -> ContextSourceKind {
        self.part.source_kind()
    }

    pub fn canonical_identity_hash(&self) -> u64 {
        context_identity_hash(&self.part)
    }

    pub fn display_label(&self) -> String {
        match &self.state {
            ContextLifecycleState::Failed { .. } => {
                format!("Couldn’t add · {}", self.part.label())
            }
            ContextLifecycleState::Resolving => {
                format!(
                    "{} · {} · Preparing",
                    self.provenance.cue(),
                    self.part.label()
                )
            }
            ContextLifecycleState::Pending | ContextLifecycleState::Resolved => {
                format!("{} · {}", self.provenance.cue(), self.part.label())
            }
        }
    }

    pub fn can_remove(&self) -> bool {
        self.removable && self.lifetime == ContextLifetime::NextTurn
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageContextItemOutcome {
    Added { index: usize },
    Upgraded { index: usize },
    Duplicate { index: usize },
}

/// Merge one item into pending context while preserving the earliest visible
/// position and stable ID. Primary wins over supplemental. Within one role,
/// provenance priority is UserMention > AttachmentPortal > HostHandoff >
/// ImplicitFocused > DeferredAmbient.
pub fn stage_context_item(
    items: &mut Vec<StagedContextItem>,
    incoming: StagedContextItem,
) -> StageContextItemOutcome {
    debug_assert_eq!(incoming.lifetime, ContextLifetime::NextTurn);
    let identity = incoming.canonical_identity_hash();
    let Some(index) = items
        .iter()
        .position(|existing| existing.canonical_identity_hash() == identity)
    else {
        let index = items.len();
        items.push(incoming);
        return StageContextItemOutcome::Added { index };
    };

    let existing = &mut items[index];
    let incoming_wins = incoming.role.priority() > existing.role.priority()
        || (incoming.role == existing.role
            && incoming.provenance.pending_priority() > existing.provenance.pending_priority());

    if incoming_wins {
        existing.part = incoming.part;
        existing.provenance = incoming.provenance;
        existing.role = incoming.role;
        existing.state = ContextLifecycleState::Pending;
        existing.removable = incoming.removable;
        StageContextItemOutcome::Upgraded { index }
    } else {
        StageContextItemOutcome::Duplicate { index }
    }
}

fn context_identity_hash(part: &AiContextPart) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in source_kind_label(part.source_kind())
        .bytes()
        .chain([0])
        .chain(part.source().bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn source_kind_label(kind: ContextSourceKind) -> &'static str {
    match kind {
        ContextSourceKind::Resource => "resource",
        ContextSourceKind::File => "file",
        ContextSourceKind::Skill => "skill",
        ContextSourceKind::FocusedTarget => "focused",
        ContextSourceKind::Ambient => "ambient",
        ContextSourceKind::Text => "text",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(label: &str) -> AiContextPart {
        AiContextPart::FilePath {
            path: "/tmp/context.txt".to_string(),
            label: label.to_string(),
        }
    }

    #[test]
    fn dedupe_uses_canonical_identity_not_label_or_full_equality() {
        let mut items = vec![StagedContextItem::pending(
            file("first label"),
            ContextProvenance::ImplicitFocused,
            ContextRole::Supplemental,
        )];
        let original_id = items[0].id.clone();
        let outcome = stage_context_item(
            &mut items,
            StagedContextItem::pending(
                file("portal label"),
                ContextProvenance::AttachmentPortal,
                ContextRole::Supplemental,
            ),
        );
        assert_eq!(outcome, StageContextItemOutcome::Upgraded { index: 0 });
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, original_id);
        assert_eq!(items[0].part.label(), "portal label");
        assert_eq!(items[0].provenance, ContextProvenance::AttachmentPortal);
    }

    #[test]
    fn primary_wins_without_moving_the_earliest_visible_item() {
        let mut items = vec![StagedContextItem::pending(
            file("supplemental"),
            ContextProvenance::UserMention,
            ContextRole::Supplemental,
        )];
        let original_id = items[0].id.clone();
        let outcome = stage_context_item(
            &mut items,
            StagedContextItem::pending(
                file("required"),
                ContextProvenance::HostHandoff,
                ContextRole::Primary,
            ),
        );
        assert_eq!(outcome, StageContextItemOutcome::Upgraded { index: 0 });
        assert_eq!(items[0].id, original_id);
        assert_eq!(items[0].role, ContextRole::Primary);
        assert_eq!(items[0].provenance, ContextProvenance::HostHandoff);
    }

    #[test]
    fn thread_receipt_is_immutable_and_not_removable() {
        let receipt = StagedContextItem::pending(
            file("sent"),
            ContextProvenance::UserMention,
            ContextRole::Supplemental,
        )
        .immutable_receipt_from();
        assert_eq!(receipt.provenance, ContextProvenance::ThreadReceipt);
        assert_eq!(receipt.lifetime, ContextLifetime::ImmutableReceipt);
        assert_eq!(receipt.state, ContextLifecycleState::Resolved);
        assert!(!receipt.can_remove());
    }

    #[test]
    fn redacted_debug_omits_path_and_label() {
        let item = StagedContextItem::pending(
            AiContextPart::FilePath {
                path: "/private/CONTEXT_PATH_CANARY".to_string(),
                label: "CONTEXT_LABEL_CANARY".to_string(),
            },
            ContextProvenance::HostHandoff,
            ContextRole::Primary,
        );
        let debug = format!("{item:?}");
        assert!(!debug.contains("CONTEXT_PATH_CANARY"));
        assert!(!debug.contains("CONTEXT_LABEL_CANARY"));
    }
}
