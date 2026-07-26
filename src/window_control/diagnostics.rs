//! Structured diagnostics receipts for window-engine operations.
//!
//! Every engine operation records one `WindowOperationDiagnostics` into a
//! bounded in-process ring (newest 256). Timings use microseconds so the
//! sub-millisecond geometry budget is measurable. Receipts never carry full
//! window titles, document paths, or raw AX pointers — only identity-safe
//! numeric fields and warning/error codes.

use std::collections::VecDeque;

/// Where an operation originated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationSource {
    LegacyAction,
    Protocol,
    SnapSession,
    WindowSwitcher,
    Undo,
    Redo,
    Test,
}

/// Which backend executed an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationBackend {
    Ax,
    TestProvider,
}

/// Terminal status of an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationStatus {
    Succeeded,
    Partial,
    Failed,
    TimedOut,
    RolledBack,
    RollbackFailed,
    Cancelled,
}

/// Registry cache disposition for the operation's target resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CacheStatus {
    Hit,
    Refreshed,
    Miss,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowOperationDiagnostics {
    pub schema_version: u8,
    pub correlation_id: String,
    pub plan_id: Option<String>,
    pub source: OperationSource,
    pub backend: OperationBackend,
    pub status: OperationStatus,
    pub legacy_window_id: Option<u32>,
    pub handle_nonce: Option<u64>,
    pub registry_generation: Option<u64>,
    pub topology_generation: Option<u64>,
    pub observation_us: u64,
    pub planning_us: u64,
    pub queue_wait_us: u64,
    pub first_mutation_us: Option<u64>,
    pub mutation_us: u64,
    pub verification_us: u64,
    pub rollback_us: u64,
    pub total_us: u64,
    pub attempts: u8,
    pub readback_count: u8,
    pub cache_status: CacheStatus,
    pub ax_timeout_ms: Option<u64>,
    pub warning_codes: Vec<String>,
    pub error_code: Option<String>,
}

const DIAGNOSTICS_DEPTH: usize = 256;

static DIAGNOSTICS: std::sync::LazyLock<parking_lot::Mutex<VecDeque<WindowOperationDiagnostics>>> =
    std::sync::LazyLock::new(|| parking_lot::Mutex::new(VecDeque::with_capacity(DIAGNOSTICS_DEPTH)));

pub(crate) fn record_diagnostics(receipt: WindowOperationDiagnostics) {
    let mut entries = DIAGNOSTICS.lock();
    if entries.len() == DIAGNOSTICS_DEPTH {
        entries.pop_front();
    }
    entries.push_back(receipt);
}

/// Snapshot of the retained diagnostics, oldest first.
#[doc(hidden)]
pub fn window_operation_diagnostics_snapshot() -> Vec<WindowOperationDiagnostics> {
    DIAGNOSTICS.lock().iter().cloned().collect()
}

/// Allocate a process-unique correlation id.
pub(crate) fn next_correlation_id() -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let sequence = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("winop-{sequence}")
}

#[cfg(test)]
pub(crate) fn clear_diagnostics_for_tests() {
    DIAGNOSTICS.lock().clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt(sequence: u64) -> WindowOperationDiagnostics {
        WindowOperationDiagnostics {
            schema_version: 1,
            correlation_id: format!("test-{sequence}"),
            plan_id: None,
            source: OperationSource::Test,
            backend: OperationBackend::TestProvider,
            status: OperationStatus::Succeeded,
            legacy_window_id: Some(1),
            handle_nonce: Some(sequence),
            registry_generation: Some(1),
            topology_generation: None,
            observation_us: 10,
            planning_us: 20,
            queue_wait_us: 0,
            first_mutation_us: Some(30),
            mutation_us: 40,
            verification_us: 5,
            rollback_us: 0,
            total_us: 105,
            attempts: 1,
            readback_count: 1,
            cache_status: CacheStatus::Hit,
            ax_timeout_ms: None,
            warning_codes: Vec::new(),
            error_code: None,
        }
    }

    #[test]
    fn ring_retains_newest_256_entries_in_order() {
        clear_diagnostics_for_tests();
        for sequence in 0..300u64 {
            record_diagnostics(receipt(sequence));
        }
        let snapshot = window_operation_diagnostics_snapshot();
        assert_eq!(snapshot.len(), 256);
        assert_eq!(snapshot.first().unwrap().handle_nonce, Some(44));
        assert_eq!(snapshot.last().unwrap().handle_nonce, Some(299));
        clear_diagnostics_for_tests();
    }

    #[test]
    fn serialized_receipt_contains_no_title_or_pointer_fields() {
        let json = serde_json::to_value(receipt(1)).expect("serialize");
        let object = json.as_object().expect("object");
        for forbidden in ["title", "documentPath", "axWindow", "axPointer", "path"] {
            assert!(
                !object.contains_key(forbidden),
                "diagnostics must not expose {forbidden}"
            );
        }
        assert!(object.contains_key("correlationId"));
        assert!(object.contains_key("firstMutationUs"));
    }

    #[test]
    fn correlation_ids_are_unique_and_monotonic() {
        let first = next_correlation_id();
        let second = next_correlation_id();
        assert_ne!(first, second);
    }
}
