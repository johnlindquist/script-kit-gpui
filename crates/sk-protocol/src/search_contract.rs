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
        let generation = self.active.get(&source).map_or(1, |request| {
            request
                .generation
                .checked_add(1)
                .expect("provider_request_generation_exhausted")
        });
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
        let next = self.active.get(&source).map_or(1, |request| {
            request
                .generation
                .checked_add(1)
                .expect("provider_request_generation_exhausted")
        });
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

/// Exact ownership of one bounded background provider worker. Source is part
/// of the ticket, so a stale completion cannot release another provider's work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RootOwnedProviderRefresh {
    pub source: CommandSource,
    pub generation: u64,
}

#[derive(Debug, Default)]
pub struct RootOwnedProviderRefreshLifecycle {
    pub next_generation: u64,
    pub in_flight: Option<RootOwnedProviderRefresh>,
}

impl RootOwnedProviderRefreshLifecycle {
    pub fn begin(
        &mut self,
        source: CommandSource,
        cache_is_fresh: bool,
    ) -> Option<RootOwnedProviderRefresh> {
        if cache_is_fresh || self.in_flight.is_some() {
            return None;
        }
        self.next_generation = self.next_generation.checked_add(1)?;
        let refresh = RootOwnedProviderRefresh {
            source,
            generation: self.next_generation,
        };
        self.in_flight = Some(refresh);
        Some(refresh)
    }

    pub fn finish(&mut self, refresh: RootOwnedProviderRefresh) -> bool {
        if self.in_flight != Some(refresh) {
            return false;
        }
        self.in_flight = None;
        true
    }
}

/// One source-owned query coordinator shared by app launcher/provider adapters.
#[derive(Debug, Default)]
pub struct RootProviderCoordinator {
    generations: ProviderGenerationFence,
}

impl RootProviderCoordinator {
    /// Reuse an exact active request so repeated renders cannot duplicate work.
    pub fn begin(&mut self, source: CommandSource, query: &str) -> ProviderRequest {
        if let Some(current) = self.generations.current(source) {
            if current.query == query {
                return current.clone();
            }
        }
        self.generations.begin(source, query)
    }

    /// Provider completion, generation lineage, and live input must all agree.
    pub fn accepts(&self, request: &ProviderRequest, current_query: &str) -> bool {
        request.query == current_query && self.generations.accepts(request)
    }

    pub fn invalidate(&mut self, source: CommandSource) {
        self.generations.invalidate(source);
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
    #[should_panic(expected = "provider_request_generation_exhausted")]
    fn provider_request_exhaustion_refuses_generation_reuse() {
        let mut fence = ProviderGenerationFence::default();
        fence.active.insert(
            CommandSource::BrowserTab,
            ProviderRequest {
                source: CommandSource::BrowserTab,
                generation: u64::MAX,
                query: "docs".into(),
            },
        );
        fence.begin(CommandSource::BrowserTab, "docs");
    }

    #[test]
    #[should_panic(expected = "provider_request_generation_exhausted")]
    fn provider_request_invalidation_exhaustion_refuses_generation_reuse() {
        let mut fence = ProviderGenerationFence::default();
        fence.active.insert(
            CommandSource::BrowserHistory,
            ProviderRequest {
                source: CommandSource::BrowserHistory,
                generation: u64::MAX,
                query: "docs".into(),
            },
        );
        fence.invalidate(CommandSource::BrowserHistory);
    }

    #[test]
    fn owned_provider_worker_rejects_duplicate_fresh_or_cross_source_completion() {
        let mut lifecycle = RootOwnedProviderRefreshLifecycle::default();
        assert!(lifecycle.begin(CommandSource::Clipboard, true).is_none());

        let clipboard = lifecycle
            .begin(CommandSource::Clipboard, false)
            .expect("cold clipboard owns one worker");
        assert!(lifecycle.begin(CommandSource::Clipboard, false).is_none());
        assert!(!lifecycle.finish(RootOwnedProviderRefresh {
            source: CommandSource::Dictation,
            generation: clipboard.generation,
        }));
        assert!(
            lifecycle
                .begin(CommandSource::Conversation, false)
                .is_none()
        );
        assert!(lifecycle.finish(clipboard));
    }

    #[test]
    fn owned_provider_worker_stale_completion_cannot_release_replacement() {
        let mut lifecycle = RootOwnedProviderRefreshLifecycle::default();
        let stale = lifecycle
            .begin(CommandSource::Dictation, false)
            .expect("first dictation worker");
        assert!(lifecycle.finish(stale));
        let current = lifecycle
            .begin(CommandSource::Dictation, false)
            .expect("replacement dictation worker");

        assert!(current.generation > stale.generation);
        assert!(!lifecycle.finish(stale));
        assert_eq!(lifecycle.in_flight, Some(current));
        assert!(lifecycle.finish(current));
    }

    #[test]
    fn owned_provider_generation_exhaustion_never_reuses_a_retired_ticket() {
        let mut lifecycle = RootOwnedProviderRefreshLifecycle {
            next_generation: u64::MAX - 1,
            in_flight: None,
        };
        let last = lifecycle.begin(CommandSource::Conversation, false).unwrap();
        assert_eq!(last.generation, u64::MAX);
        assert!(lifecycle.finish(last));
        assert!(
            lifecycle
                .begin(CommandSource::Conversation, false)
                .is_none()
        );
        assert!(lifecycle.in_flight.is_none());
        assert!(!lifecycle.finish(last));
    }

    #[test]
    fn repeated_exact_query_reuses_the_existing_generation() {
        let mut coordinator = RootProviderCoordinator::default();
        let first = coordinator.begin(CommandSource::BrowserHistory, "script");
        let second = coordinator.begin(CommandSource::BrowserHistory, "script");

        assert_eq!(first, second);
        assert!(coordinator.accepts(&first, "script"));
    }

    #[test]
    fn stale_provider_batches_cannot_replace_a_newer_query() {
        let mut coordinator = RootProviderCoordinator::default();
        let stale = coordinator.begin(CommandSource::BrowserTab, "s");
        let current = coordinator.begin(CommandSource::BrowserTab, "script");

        assert!(current.generation > stale.generation);
        assert!(!coordinator.accepts(&stale, "script"));
        assert!(!coordinator.accepts(&current, "s"));
        assert!(coordinator.accepts(&current, "script"));
    }

    #[test]
    fn clearing_a_query_invalidates_its_inflight_provider_response() {
        let mut coordinator = RootProviderCoordinator::default();
        let pending = coordinator.begin(CommandSource::BrowserHistory, "docs");
        coordinator.invalidate(CommandSource::BrowserHistory);

        assert!(!coordinator.accepts(&pending, "docs"));
    }

    #[test]
    fn one_passive_provider_cannot_cancel_another_providers_results() {
        let mut coordinator = RootProviderCoordinator::default();
        let tabs = coordinator.begin(CommandSource::BrowserTab, "docs");
        let history = coordinator.begin(CommandSource::BrowserHistory, "docs");

        coordinator.invalidate(CommandSource::BrowserTab);

        assert!(!coordinator.accepts(&tabs, "docs"));
        assert!(coordinator.accepts(&history, "docs"));
    }

    #[test]
    fn same_text_from_the_wrong_provider_or_generation_is_refused() {
        let mut coordinator = RootProviderCoordinator::default();
        let expected = coordinator.begin(CommandSource::Clipboard, "hello");
        let wrong_source = ProviderRequest {
            source: CommandSource::Dictation,
            ..expected.clone()
        };
        let wrong_generation = ProviderRequest {
            generation: expected.generation.wrapping_add(1),
            ..expected.clone()
        };

        assert!(!coordinator.accepts(&wrong_source, "hello"));
        assert!(!coordinator.accepts(&wrong_generation, "hello"));
        assert!(coordinator.accepts(&expected, "hello"));
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
