use super::capabilities::{CapabilityEvidence, ExecutableIdentity};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sk_protocol::ai_reliability::{ClientKind, ModelId, ProviderId};
use std::collections::{HashMap, VecDeque};

const MAX_CAPABILITY_RECORDS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CapabilityCacheKey {
    pub executable_canonical_id: String,
    pub executable_fingerprint: String,
    pub provider_id: ProviderId,
    pub model_id: ModelId,
}

impl CapabilityCacheKey {
    pub fn new(
        executable: &ExecutableIdentity,
        provider_id: ProviderId,
        model_id: ModelId,
    ) -> Self {
        Self {
            executable_canonical_id: executable.canonical_id.clone(),
            executable_fingerprint: executable.fingerprint.clone(),
            provider_id,
            model_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompatibilityRecordKind {
    ClientTooOld { client: ClientKind },
    LastSuccessful,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompatibilityRecord {
    pub key: CapabilityCacheKey,
    pub kind: CompatibilityRecordKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedCapabilityRecords {
    pub records: Vec<CompatibilityRecord>,
}

#[derive(Debug, Default)]
struct CacheState {
    records: HashMap<CapabilityCacheKey, CompatibilityRecordKind>,
    insertion_order: VecDeque<CapabilityCacheKey>,
}

/// Bounded in-memory capability snapshot.
///
/// Process/version probing happens outside this type. Submission-time Quick AI
/// reads are a single bounded `RwLock<HashMap>` lookup and never spawn.
#[derive(Debug, Default)]
pub struct CapabilityCache {
    state: RwLock<CacheState>,
}

impl CapabilityCache {
    pub fn restore(&self, persisted: PersistedCapabilityRecords) {
        for record in persisted.records {
            self.insert(record);
        }
    }

    pub fn record_negative(
        &self,
        key: CapabilityCacheKey,
        client: ClientKind,
    ) -> CompatibilityRecord {
        self.insert(CompatibilityRecord {
            key,
            kind: CompatibilityRecordKind::ClientTooOld { client },
        })
    }

    pub fn record_success(&self, key: CapabilityCacheKey) -> CompatibilityRecord {
        self.insert(CompatibilityRecord {
            key,
            kind: CompatibilityRecordKind::LastSuccessful,
        })
    }

    pub fn snapshot(
        &self,
        executable: ExecutableIdentity,
        provider_id: ProviderId,
        model_id: ModelId,
    ) -> CapabilityEvidence {
        let key = CapabilityCacheKey::new(&executable, provider_id, model_id);
        let record = self.state.read().records.get(&key).cloned();
        CapabilityEvidence {
            executable,
            advertised_models: Vec::new(),
            exact_record: record,
            roster_protocol_ready: None,
            spawned_processes: 0,
        }
    }

    pub fn persisted(&self) -> PersistedCapabilityRecords {
        let state = self.state.read();
        PersistedCapabilityRecords {
            records: state
                .records
                .iter()
                .map(|(key, kind)| CompatibilityRecord {
                    key: key.clone(),
                    kind: kind.clone(),
                })
                .collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.state.read().records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.state.read().records.is_empty()
    }

    fn insert(&self, record: CompatibilityRecord) -> CompatibilityRecord {
        let mut state = self.state.write();
        if !state.records.contains_key(&record.key) {
            state.insertion_order.push_back(record.key.clone());
        }
        state
            .records
            .insert(record.key.clone(), record.kind.clone());
        while state.records.len() > MAX_CAPABILITY_RECORDS {
            if let Some(oldest) = state.insertion_order.pop_front() {
                state.records.remove(&oldest);
            }
        }
        record
    }
}
