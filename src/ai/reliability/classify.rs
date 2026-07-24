use super::diagnostics::{safe_parameters, DiagnosticVault};
use sk_protocol::ai_reliability::{
    AiFailure, AiFailureCode, AiFailureKind, AuthenticationFailure, CapabilityFailure, ClientKind,
    ConfigurationFailure, ConnectivityFailure, InputFailure, ModelAvailabilityReason, ModelId,
    PermissionFailure, PolicyFailure, ProtocolComponent, ProtocolFailure, ProviderFailure,
    ReattachAvailability, RetrySafety, RuntimeFailure, SessionRef, ToolId,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureContext {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub component: ProtocolComponent,
    pub http_status: Option<u16>,
    pub retry_after_ms: Option<u64>,
}

impl Default for FailureContext {
    fn default() -> Self {
        Self {
            provider: None,
            model: None,
            component: ProtocolComponent::Provider,
            http_status: None,
            retry_after_ms: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailurePresentationInput {
    pub title_key: &'static str,
    pub message_key: &'static str,
    pub parameters: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppFailureRecord {
    pub failure: AiFailure,
    pub presentation: FailurePresentationInput,
}

impl AppFailureRecord {
    pub fn primary_message(&self) -> &'static str {
        primary_message_for_failure(&self.failure)
    }
}

pub fn primary_message_for_failure(failure: &AiFailure) -> &'static str {
    match failure.code {
        AiFailureCode::ClientTooOld => {
            "This AI client needs an update before it can use the selected model."
        }
        AiFailureCode::ModelUnavailable | AiFailureCode::NoCompatibleModel => {
            "The selected model is not available. Choose a compatible model to continue."
        }
        AiFailureCode::ProfileUnavailable => {
            "The selected AI profile is no longer available. Choose another profile."
        }
        AiFailureCode::QuickAiSearchBudgetExceeded => {
            "Quick AI reached its search limit. Your question and current results are saved."
        }
        AiFailureCode::AuthenticationMissing | AiFailureCode::AuthenticationExpired => {
            "Sign in to continue with this AI provider."
        }
        AiFailureCode::UsageExhausted => "This account has reached its current usage limit.",
        AiFailureCode::ProviderNotConfigured
        | AiFailureCode::NoModelsAvailable
        | AiFailureCode::SidecarMissing
        | AiFailureCode::MdflowMissing
        | AiFailureCode::InvalidConfiguration => {
            "This AI integration needs setup before it can continue."
        }
        AiFailureCode::Offline
        | AiFailureCode::Timeout
        | AiFailureCode::RateLimited
        | AiFailureCode::ProviderTemporarilyUnavailable => {
            "The AI service is temporarily unavailable. Your work is saved."
        }
        AiFailureCode::ProviderServerRejected => {
            "The AI service could not accept this request. Your work is saved."
        }
        AiFailureCode::SpawnFailed
        | AiFailureCode::RuntimeClosed
        | AiFailureCode::ChildExited
        | AiFailureCode::SessionLost => {
            "The AI connection stopped. Your work is saved and can be recovered."
        }
        AiFailureCode::ProtocolVersionMismatch
        | AiFailureCode::ProtocolSequenceViolation
        | AiFailureCode::ProtocolOrderViolation
        | AiFailureCode::ProtocolMalformedResponse
        | AiFailureCode::ProtocolMissingTerminal => {
            "This AI component is incompatible or returned an invalid response."
        }
        AiFailureCode::PermissionDenied
        | AiFailureCode::UserDeniedTool
        | AiFailureCode::ToolDenied => "This AI action needs permission before it can continue.",
        AiFailureCode::MessageTooLarge | AiFailureCode::ContextLimitExceeded => {
            "This request is too large. Shorten it or remove some context."
        }
        AiFailureCode::Unknown => {
            "The AI request did not finish. Your work is saved; try again or view details."
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessFailureFacts {
    SpawnFailed,
    RuntimeClosed,
    ChildExited {
        exit_code: Option<i32>,
        signal: Option<i32>,
    },
    SessionLost {
        session: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolFailureFacts {
    VersionMismatch {
        expected: String,
        actual: Option<String>,
    },
    SequenceViolation,
    OrderViolation,
    MalformedResponse,
    MissingTerminal,
}

pub fn classify_provider_failure(
    context: &FailureContext,
    raw: &str,
    diagnostics: &DiagnosticVault,
) -> AppFailureRecord {
    let normalized = raw.to_ascii_lowercase();

    // Order is a compatibility contract. Precise protocol and capability
    // evidence must win over broad provider/status wording.
    let kind = if has_protocol_version_evidence(&normalized) {
        AiFailureKind::Protocol(ProtocolFailure::VersionMismatch {
            component: context.component,
            expected: "supported".to_string(),
            actual: extract_quoted_value(raw, "actual"),
        })
    } else if contains_any(
        &normalized,
        &["sequence violation", "sequence_error", "seq mismatch"],
    ) {
        AiFailureKind::Protocol(ProtocolFailure::SequenceViolation {
            component: context.component,
        })
    } else if contains_any(
        &normalized,
        &["order violation", "out of order", "unexpected event order"],
    ) {
        AiFailureKind::Protocol(ProtocolFailure::OrderViolation {
            component: context.component,
        })
    } else if requires_newer_client(&normalized) {
        AiFailureKind::Capability(CapabilityFailure::ClientTooOld {
            client: client_for_failure_text(&normalized, context.component),
            model: context
                .model
                .clone()
                .or_else(|| extract_model(raw))
                .map(ModelId),
        })
    } else if normalized.contains("quick_ai_more_than_two_search_queries") {
        AiFailureKind::Policy(PolicyFailure::QuickAiSearchBudgetExceeded {
            completed_searches: 2,
            budget: 2,
            partial_answer_available: true,
            source_count: 2,
        })
    } else if contains_any(
        &normalized,
        &[
            "usage_limit_reached",
            "usage exhausted",
            "quota exhausted",
            "credit balance is too low",
        ],
    ) {
        AiFailureKind::Authentication(AuthenticationFailure::UsageExhausted)
    } else if context.http_status == Some(401)
        || contains_any(
            &normalized,
            &[
                "authentication required",
                "unauthorized",
                "invalid api key",
                "missing api key",
                "no api key",
            ],
        )
    {
        AiFailureKind::Authentication(AuthenticationFailure::Missing)
    } else if contains_any(
        &normalized,
        &["authentication expired", "session expired", "token expired"],
    ) {
        AiFailureKind::Authentication(AuthenticationFailure::Expired)
    } else if contains_any(
        &normalized,
        &["provider not configured", "missing provider configuration"],
    ) {
        AiFailureKind::Configuration(ConfigurationFailure::ProviderNotConfigured)
    } else if contains_any(&normalized, &["no models available", "model list is empty"]) {
        AiFailureKind::Configuration(ConfigurationFailure::NoModelsAvailable)
    } else if contains_any(&normalized, &["sidecar missing", "sidecar not found"]) {
        AiFailureKind::Configuration(ConfigurationFailure::SidecarMissing)
    } else if contains_any(&normalized, &["mdflow missing", "mdflow not found"]) {
        AiFailureKind::Configuration(ConfigurationFailure::MdflowMissing)
    } else if contains_any(
        &normalized,
        &["invalid configuration", "configuration invalid"],
    ) {
        AiFailureKind::Configuration(ConfigurationFailure::InvalidConfiguration)
    } else if contains_any(
        &normalized,
        &["permission denied", "operation not permitted"],
    ) {
        AiFailureKind::Permission(PermissionFailure::PermissionDenied)
    } else if contains_any(&normalized, &["tool denied", "user denied tool"]) {
        AiFailureKind::Permission(PermissionFailure::UserDeniedTool)
    } else if contains_any(
        &normalized,
        &[
            "context length",
            "context limit",
            "maximum context",
            "too many tokens",
        ],
    ) {
        AiFailureKind::Input(InputFailure::ContextLimitExceeded)
    } else if contains_any(
        &normalized,
        &["message too large", "request body too large"],
    ) {
        AiFailureKind::Input(InputFailure::MessageTooLarge)
    } else if contains_any(&normalized, &["timed out", "timeout"]) {
        AiFailureKind::Connectivity(ConnectivityFailure::Timeout)
    } else if contains_any(
        &normalized,
        &["offline", "network unreachable", "not connected"],
    ) {
        AiFailureKind::Connectivity(ConnectivityFailure::Offline)
    } else if context.http_status == Some(429)
        || contains_any(
            &normalized,
            &["rate limit", "rate_limit", "too many requests"],
        )
    {
        AiFailureKind::Connectivity(ConnectivityFailure::RateLimited {
            retry_after_ms: context.retry_after_ms,
        })
    } else if context.http_status.is_some_and(|status| status >= 500)
        || contains_any(
            &normalized,
            &[
                "provider outage",
                "service unavailable",
                "temporarily unavailable",
            ],
        )
    {
        AiFailureKind::Provider(ProviderFailure::TemporarilyUnavailable)
    } else if contains_any(
        &normalized,
        &[
            "model unavailable",
            "model not found",
            "does not exist",
            "unsupported model",
        ],
    ) {
        AiFailureKind::Capability(CapabilityFailure::ModelUnavailable {
            model: ModelId(
                context
                    .model
                    .clone()
                    .or_else(|| extract_model(raw))
                    .unwrap_or_else(|| "unknown".to_string()),
            ),
            reason: ModelAvailabilityReason::NotAdvertised,
        })
    } else if context
        .http_status
        .is_some_and(|status| (400..500).contains(&status))
        || contains_any(&normalized, &["invalid_request_error", "invalid request"])
    {
        AiFailureKind::Provider(ProviderFailure::ServerRejected)
    } else {
        AiFailureKind::Unknown
    };

    record(context, raw, diagnostics, kind)
}

pub fn classify_process_failure(
    context: &FailureContext,
    facts: ProcessFailureFacts,
    diagnostics: &DiagnosticVault,
) -> AppFailureRecord {
    let (kind, raw) = match facts {
        ProcessFailureFacts::SpawnFailed => (
            AiFailureKind::Runtime(RuntimeFailure::SpawnFailed),
            "process:spawn_failed".to_string(),
        ),
        ProcessFailureFacts::RuntimeClosed => (
            AiFailureKind::Runtime(RuntimeFailure::RuntimeClosed),
            "process:runtime_closed".to_string(),
        ),
        ProcessFailureFacts::ChildExited { exit_code, signal } => (
            AiFailureKind::Runtime(RuntimeFailure::ChildExited { exit_code, signal }),
            format!("process:child_exited code={exit_code:?} signal={signal:?}"),
        ),
        ProcessFailureFacts::SessionLost { session } => (
            AiFailureKind::Runtime(RuntimeFailure::SessionLost {
                reattach: session
                    .map(|session| ReattachAvailability::Available {
                        session: SessionRef(session),
                    })
                    .unwrap_or(ReattachAvailability::Unavailable),
            }),
            "process:session_lost".to_string(),
        ),
    };
    record(context, &raw, diagnostics, kind)
}

pub fn classify_protocol_failure(
    context: &FailureContext,
    facts: ProtocolFailureFacts,
    diagnostics: &DiagnosticVault,
) -> AppFailureRecord {
    let kind = match facts {
        ProtocolFailureFacts::VersionMismatch { expected, actual } => {
            AiFailureKind::Protocol(ProtocolFailure::VersionMismatch {
                component: context.component,
                expected,
                actual,
            })
        }
        ProtocolFailureFacts::SequenceViolation => {
            AiFailureKind::Protocol(ProtocolFailure::SequenceViolation {
                component: context.component,
            })
        }
        ProtocolFailureFacts::OrderViolation => {
            AiFailureKind::Protocol(ProtocolFailure::OrderViolation {
                component: context.component,
            })
        }
        ProtocolFailureFacts::MalformedResponse => {
            AiFailureKind::Protocol(ProtocolFailure::MalformedResponse {
                component: context.component,
            })
        }
        ProtocolFailureFacts::MissingTerminal => {
            AiFailureKind::Protocol(ProtocolFailure::MissingTerminal {
                component: context.component,
            })
        }
    };
    record(context, "typed protocol failure", diagnostics, kind)
}

fn record(
    context: &FailureContext,
    raw: &str,
    diagnostics: &DiagnosticVault,
    kind: AiFailureKind,
) -> AppFailureRecord {
    let descriptor = diagnostics.capture(raw);
    let retry_safety = retry_safety(&kind);
    let failure = AiFailure::new(kind, retry_safety).with_diagnostic(descriptor);
    AppFailureRecord {
        presentation: presentation(context, &failure),
        failure,
    }
}

fn presentation(context: &FailureContext, failure: &AiFailure) -> FailurePresentationInput {
    let (title_key, message_key) = match failure.code {
        AiFailureCode::ClientTooOld => (
            "ai.client_update_required",
            "ai.client_update_required.detail",
        ),
        AiFailureCode::ModelUnavailable | AiFailureCode::NoCompatibleModel => {
            ("ai.model_unavailable", "ai.model_unavailable.detail")
        }
        AiFailureCode::QuickAiSearchBudgetExceeded => {
            ("ai.quick_search_limit", "ai.quick_search_limit.detail")
        }
        AiFailureCode::AuthenticationMissing | AiFailureCode::AuthenticationExpired => {
            ("ai.sign_in_required", "ai.sign_in_required.detail")
        }
        AiFailureCode::UsageExhausted => ("ai.usage_exhausted", "ai.usage_exhausted.detail"),
        AiFailureCode::ProviderNotConfigured
        | AiFailureCode::NoModelsAvailable
        | AiFailureCode::SidecarMissing
        | AiFailureCode::MdflowMissing
        | AiFailureCode::InvalidConfiguration => ("ai.setup_required", "ai.setup_required.detail"),
        AiFailureCode::Offline | AiFailureCode::Timeout => {
            ("ai.connection_failed", "ai.connection_failed.detail")
        }
        AiFailureCode::RateLimited => ("ai.rate_limited", "ai.rate_limited.detail"),
        AiFailureCode::ProviderTemporarilyUnavailable | AiFailureCode::ProviderServerRejected => {
            ("ai.provider_failed", "ai.provider_failed.detail")
        }
        AiFailureCode::SpawnFailed
        | AiFailureCode::RuntimeClosed
        | AiFailureCode::ChildExited
        | AiFailureCode::SessionLost => ("ai.runtime_failed", "ai.runtime_failed.detail"),
        AiFailureCode::ProtocolVersionMismatch
        | AiFailureCode::ProtocolSequenceViolation
        | AiFailureCode::ProtocolOrderViolation
        | AiFailureCode::ProtocolMalformedResponse
        | AiFailureCode::ProtocolMissingTerminal => (
            "ai.component_incompatible",
            "ai.component_incompatible.detail",
        ),
        AiFailureCode::PermissionDenied
        | AiFailureCode::UserDeniedTool
        | AiFailureCode::ToolDenied => ("ai.permission_required", "ai.permission_required.detail"),
        AiFailureCode::MessageTooLarge | AiFailureCode::ContextLimitExceeded => {
            ("ai.input_too_large", "ai.input_too_large.detail")
        }
        AiFailureCode::ProfileUnavailable => {
            ("ai.profile_unavailable", "ai.profile_unavailable.detail")
        }
        AiFailureCode::Unknown => ("ai.turn_failed", "ai.turn_failed.detail"),
    };
    FailurePresentationInput {
        title_key,
        message_key,
        parameters: safe_parameters([
            ("provider", context.provider.clone()),
            ("model", context.model.clone()),
        ]),
    }
}

fn retry_safety(kind: &AiFailureKind) -> RetrySafety {
    match kind {
        AiFailureKind::Connectivity(ConnectivityFailure::Offline)
        | AiFailureKind::Connectivity(ConnectivityFailure::Timeout)
        | AiFailureKind::Connectivity(ConnectivityFailure::RateLimited { .. })
        | AiFailureKind::Provider(ProviderFailure::TemporarilyUnavailable) => {
            RetrySafety::SameSelectionReadOnly
        }
        AiFailureKind::Runtime(RuntimeFailure::SessionLost {
            reattach: ReattachAvailability::Available { .. },
        }) => RetrySafety::ReconnectOnly,
        AiFailureKind::Capability(_)
        | AiFailureKind::Policy(_)
        | AiFailureKind::Authentication(_)
        | AiFailureKind::Configuration(_)
        | AiFailureKind::Provider(ProviderFailure::ServerRejected)
        | AiFailureKind::Runtime(_)
        | AiFailureKind::Protocol(_)
        | AiFailureKind::Permission(_)
        | AiFailureKind::Input(_)
        | AiFailureKind::Unknown => RetrySafety::ExplicitUserConfirmation,
    }
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn has_protocol_version_evidence(value: &str) -> bool {
    contains_any(
        value,
        &[
            "protocol version mismatch",
            "unsupported protocol version",
            "protocol_version_mismatch",
        ],
    )
}

fn requires_newer_client(value: &str) -> bool {
    (value.contains("requires a newer version")
        || value.contains("client is too old")
        || value.contains("upgrade to the latest app or cli"))
        && contains_any(value, &["codex", "pi", "mdflow", "client", "app", "cli"])
}

fn client_for_component(component: ProtocolComponent) -> ClientKind {
    match component {
        ProtocolComponent::Codex => ClientKind::Codex,
        ProtocolComponent::Pi => ClientKind::Pi,
        ProtocolComponent::Mdflow => ClientKind::Mdflow,
        ProtocolComponent::LocalLlm => ClientKind::LocalLlm,
        ProtocolComponent::Provider => ClientKind::Other,
    }
}

fn client_for_failure_text(value: &str, component: ProtocolComponent) -> ClientKind {
    if value.contains("codex") {
        ClientKind::Codex
    } else if value.contains("mdflow") {
        ClientKind::Mdflow
    } else if value.contains(" pi ") || value.starts_with("pi ") {
        ClientKind::Pi
    } else {
        client_for_component(component)
    }
}

fn extract_model(raw: &str) -> Option<String> {
    for marker in ["' model requires", "\" model requires", " model requires"] {
        let before = raw.split(marker).next()?;
        let candidate = before
            .rsplit(|character: char| {
                character.is_whitespace() || character == '\'' || character == '"'
            })
            .next()?
            .trim_matches(|character: char| character == '\'' || character == '"');
        if !candidate.is_empty() {
            return Some(candidate.to_string());
        }
    }
    None
}

fn extract_quoted_value(raw: &str, key: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(raw).ok()?;
    value.get(key)?.as_str().map(str::to_string)
}
