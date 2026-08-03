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

pub(crate) fn context_unavailable_failure(detail: &str) -> AppFailureRecord {
    super::classify_context_unavailable(
        &FailureContext {
            component: ProtocolComponent::Provider,
            ..FailureContext::default()
        },
        detail,
        runtime_vault(),
    )
}

/// Typed classification for a Quick AI turn failure, falling back to the
/// free-text provider classifier only when the code is not one we emit.
///
/// `detail` (our code plus any provider stderr) is captured for Copy Details
/// but never gets to choose the failure kind.
pub(crate) fn quick_ai_failure(
    component: ProtocolComponent,
    code: &str,
    detail: &str,
) -> AppFailureRecord {
    let context = FailureContext {
        component,
        ..FailureContext::default()
    };
    super::quick_ai_turn_failure(&context, code, detail, runtime_vault())
        .unwrap_or_else(|| classify_provider_failure(&context, detail, runtime_vault()))
}

/// Typed classification for a runtime that failed to spawn, keeping its cause
/// in the diagnostic vault. See [`super::classify_spawn_failure`].
pub(crate) fn spawn_failure(component: ProtocolComponent, cause: &str) -> AppFailureRecord {
    super::classify_spawn_failure(
        &FailureContext {
            component,
            ..FailureContext::default()
        },
        cause,
        runtime_vault(),
    )
}

/// Typed classification for an IO failure against a runtime child that has
/// gone away. See [`super::classify_runtime_closed`].
pub(crate) fn runtime_closed_failure(
    component: ProtocolComponent,
    cause: &str,
) -> AppFailureRecord {
    super::classify_runtime_closed(
        &FailureContext {
            component,
            ..FailureContext::default()
        },
        cause,
        runtime_vault(),
    )
}

/// Typed classification for the runtime's `SetupRequired` event.
///
/// See [`super::classify_setup_required`]: the event is a fact, so it must not
/// be re-derived by pattern-matching the prose we would have printed.
pub(crate) fn setup_required_failure(
    component: ProtocolComponent,
    reason: &str,
    auth_methods: &[String],
) -> AppFailureRecord {
    super::classify_setup_required(
        &FailureContext {
            component,
            ..FailureContext::default()
        },
        reason,
        auth_methods,
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
