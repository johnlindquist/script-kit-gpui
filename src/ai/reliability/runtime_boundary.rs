use super::{
    classify_process_failure, classify_protocol_failure, classify_provider_failure,
    AppFailureRecord, DiagnosticVault, FailureContext, ProcessFailureFacts, ProtocolFailureFacts,
};
use sk_protocol::ai_reliability::{AiFailureCode, ProtocolComponent};
use std::sync::OnceLock;

fn runtime_vault() -> &'static DiagnosticVault {
    static VAULT: OnceLock<DiagnosticVault> = OnceLock::new();
    VAULT.get_or_init(DiagnosticVault::default)
}

pub(crate) fn provider_failure(
    component: ProtocolComponent,
    raw: impl AsRef<str>,
) -> AppFailureRecord {
    classify_provider_failure(
        &FailureContext {
            component,
            ..FailureContext::default()
        },
        raw.as_ref(),
        runtime_vault(),
    )
}

pub(crate) fn process_failure(
    component: ProtocolComponent,
    facts: ProcessFailureFacts,
) -> AppFailureRecord {
    classify_process_failure(
        &FailureContext {
            component,
            ..FailureContext::default()
        },
        facts,
        runtime_vault(),
    )
}

pub(crate) fn protocol_failure(
    component: ProtocolComponent,
    facts: ProtocolFailureFacts,
) -> AppFailureRecord {
    classify_protocol_failure(
        &FailureContext {
            component,
            ..FailureContext::default()
        },
        facts,
        runtime_vault(),
    )
}

/// Immediate failure returned before an event stream exists.
///
/// Display/logging is intentionally the stable code only. The original error
/// is captured through the diagnostic boundary and never becomes primary UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AiAdapterError {
    pub failure: AppFailureRecord,
}

impl AiAdapterError {
    pub(crate) fn from_record(failure: AppFailureRecord) -> Self {
        Self { failure }
    }

    pub(crate) fn code(&self) -> AiFailureCode {
        self.failure.failure.code
    }
}

impl std::fmt::Display for AiAdapterError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "ai_adapter_error:{:?}", self.code())
    }
}

impl std::error::Error for AiAdapterError {}

impl From<anyhow::Error> for AiAdapterError {
    fn from(error: anyhow::Error) -> Self {
        Self::from_record(provider_failure(
            ProtocolComponent::Provider,
            error.to_string(),
        ))
    }
}

pub(crate) type AiAdapterResult<T> = Result<T, AiAdapterError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AiTurnRuntimeOutcome {
    Completed {
        stop_reason: Option<String>,
    },
    Cancelled {
        kind: sk_protocol::ai_reliability::CancellationKind,
        partial: sk_protocol::ai_reliability::PartialOutputState,
    },
}
