//! Pure, generation-safe provider snapshots and deterministic search evidence.

use crate::command_contract::{CommandIdentity, CommandSource};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRequest {
    pub source: CommandSource,
    pub generation: u64,
    pub query: String,
}

/// One monotonic request lineage per provider. A response can install only
/// when both its generation and exact query match the currently active request.
#[derive(Debug, Clone, Default)]
pub struct ProviderGenerationFence {
    active: HashMap<CommandSource, ProviderRequest>,
}

impl ProviderGenerationFence {
    pub fn begin(&mut self, source: CommandSource, query: impl Into<String>) -> ProviderRequest {
        let generation = self
            .active
            .get(&source)
            .map_or(1, |request| request.generation.wrapping_add(1));
        let request = ProviderRequest {
            source,
            generation,
            query: query.into(),
        };
        self.active.insert(source, request.clone());
        request
    }

    pub fn accepts(&self, request: &ProviderRequest) -> bool {
        self.active.get(&request.source) == Some(request)
    }

    pub fn current(&self, source: CommandSource) -> Option<&ProviderRequest> {
        self.active.get(&source)
    }

    pub fn invalidate(&mut self, source: CommandSource) {
        let next = self
            .active
            .get(&source)
            .map_or(1, |request| request.generation.wrapping_add(1));
        self.active.insert(
            source,
            ProviderRequest {
                source,
                generation: next,
                query: String::new(),
            },
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RankingField {
    Title,
    Subtitle,
    Alias,
    Keyword,
    Shortcut,
    Content,
    Source,
    Filename,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RankingEvidence {
    pub field: RankingField,
    pub score: i32,
    pub tier: i32,
    pub matched_indices: Vec<usize>,
    pub frecency_boost: i32,
    pub context_boost: i32,
}

impl RankingEvidence {
    pub const fn final_score(&self) -> i32 {
        self.score
            .saturating_add(self.frecency_boost)
            .saturating_add(self.context_boost)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchCandidate {
    pub identity: CommandIdentity,
    pub evidence: RankingEvidence,
    pub section: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchSnapshot {
    pub request: ProviderRequest,
    pub candidates: Vec<SearchCandidate>,
}

impl SearchSnapshot {
    pub fn validate(&self) -> Result<(), SearchContractError> {
        let mut ids = HashSet::new();
        for candidate in &self.candidates {
            if candidate.identity.source() != self.request.source {
                return Err(SearchContractError::WrongProvider);
            }
            if candidate.section.trim().is_empty() {
                return Err(SearchContractError::MissingSection);
            }
            if !ids.insert(candidate.identity.as_str()) {
                return Err(SearchContractError::DuplicateIdentity);
            }
        }
        Ok(())
    }

    /// Equal ranking always falls back to durable command identity, never
    /// provider completion order, row position, or hash-map iteration.
    pub fn sort_deterministically(&mut self) {
        self.candidates.sort_by(|left, right| {
            right
                .evidence
                .tier
                .cmp(&left.evidence.tier)
                .then_with(|| {
                    right
                        .evidence
                        .final_score()
                        .cmp(&left.evidence.final_score())
                })
                .then_with(|| left.identity.as_str().cmp(right.identity.as_str()))
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchContractError {
    WrongProvider,
    MissingSection,
    DuplicateIdentity,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(source: CommandSource, key: &str, score: i32) -> SearchCandidate {
        SearchCandidate {
            identity: CommandIdentity::new(source, key).unwrap(),
            evidence: RankingEvidence {
                field: RankingField::Title,
                score,
                tier: 1,
                matched_indices: vec![0],
                frecency_boost: 0,
                context_boost: 0,
            },
            section: "Results".to_owned(),
        }
    }

    #[test]
    fn stale_generation_and_wrong_query_cannot_replace_current_results() {
        let mut fence = ProviderGenerationFence::default();
        let old = fence.begin(CommandSource::BrowserHistory, "old");
        let current = fence.begin(CommandSource::BrowserHistory, "current");
        assert!(!fence.accepts(&old));
        assert!(fence.accepts(&current));
        let forged = ProviderRequest {
            query: "other".to_owned(),
            ..current
        };
        assert!(!fence.accepts(&forged));
    }

    #[test]
    fn one_provider_cannot_invalidate_or_install_another_provider() {
        let mut fence = ProviderGenerationFence::default();
        let files = fence.begin(CommandSource::File, "report");
        let notes = fence.begin(CommandSource::Note, "report");
        fence.invalidate(CommandSource::File);
        assert!(!fence.accepts(&files));
        assert!(fence.accepts(&notes));
    }

    #[test]
    fn deterministic_ranking_uses_tier_score_then_stable_identity() {
        let mut fence = ProviderGenerationFence::default();
        let request = fence.begin(CommandSource::Script, "a");
        let mut snapshot = SearchSnapshot {
            request,
            candidates: vec![
                candidate(CommandSource::Script, "z", 10),
                candidate(CommandSource::Script, "a", 10),
                candidate(CommandSource::Script, "best", 20),
            ],
        };
        snapshot.sort_deterministically();
        let actual: Vec<_> = snapshot
            .candidates
            .iter()
            .map(|entry| entry.identity.as_str())
            .collect();
        assert_eq!(actual, ["script/best", "script/a", "script/z"]);
    }

    #[test]
    fn snapshots_refuse_duplicate_ids_and_wrong_provider_results() {
        let mut fence = ProviderGenerationFence::default();
        let request = fence.begin(CommandSource::Note, "a");
        let entry = candidate(CommandSource::Note, "one", 10);
        let duplicate = SearchSnapshot {
            request: request.clone(),
            candidates: vec![entry.clone(), entry],
        };
        assert_eq!(
            duplicate.validate(),
            Err(SearchContractError::DuplicateIdentity)
        );

        let wrong_provider = SearchSnapshot {
            request,
            candidates: vec![candidate(CommandSource::File, "one", 10)],
        };
        assert_eq!(
            wrong_provider.validate(),
            Err(SearchContractError::WrongProvider)
        );
    }
}
