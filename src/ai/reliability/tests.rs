use super::*;
use sk_protocol::ai_reliability::{
    AiFailureCode, AiFailureKind, ProtocolComponent, ProtocolFailure,
};

fn context(status: Option<u16>) -> FailureContext {
    FailureContext {
        provider: Some("openai-codex".to_string()),
        model: Some("gpt-5.6-sol".to_string()),
        component: ProtocolComponent::Codex,
        http_status: status,
        retry_after_ms: Some(1_500),
    }
}

#[test]
fn client_upgrade_evidence_precedes_generic_invalid_model_and_request() {
    let vault = DiagnosticVault::default();
    let record = classify_provider_failure(
        &context(Some(400)),
        r#"{"type":"error","status":400,"error":{"type":"invalid_request_error","message":"The 'gpt-5.6-sol' model requires a newer version of Codex. Please upgrade to the latest app or CLI and try again."}}"#,
        &vault,
    );
    assert_eq!(record.failure.code, AiFailureCode::ClientTooOld);
    assert_eq!(record.presentation.title_key, "ai.client_update_required");
    assert!(!format!("{record:?}").contains("invalid_request_error"));
}

#[test]
fn structured_usage_exhaustion_precedes_http_429_rate_limit() {
    let vault = DiagnosticVault::default();
    let record = classify_provider_failure(
        &context(Some(429)),
        r#"{"code":"usage_limit_reached","message":"Monthly usage exhausted"}"#,
        &vault,
    );
    assert_eq!(record.failure.code, AiFailureCode::UsageExhausted);
}

#[test]
fn protocol_evidence_never_degrades_to_provider_failure() {
    let vault = DiagnosticVault::default();
    let record = classify_provider_failure(
        &context(Some(500)),
        "unsupported protocol version: invalid request",
        &vault,
    );
    assert!(matches!(
        record.failure.kind,
        AiFailureKind::Protocol(ProtocolFailure::VersionMismatch { .. })
    ));
}

#[test]
fn typed_process_and_protocol_facts_do_not_require_string_parsing() {
    let vault = DiagnosticVault::default();
    let process = classify_process_failure(
        &context(None),
        ProcessFailureFacts::ChildExited {
            exit_code: Some(9),
            signal: None,
        },
        &vault,
    );
    assert_eq!(process.failure.code, AiFailureCode::ChildExited);

    let protocol =
        classify_protocol_failure(&context(None), ProtocolFailureFacts::OrderViolation, &vault);
    assert_eq!(protocol.failure.code, AiFailureCode::ProtocolOrderViolation);
}

#[test]
fn redactor_allowlists_json_masks_secrets_paths_and_bounds_output() {
    let home = dirs::home_dir().expect("test requires a home directory");
    let raw = format!(
        r#"{{"status":401,"code":"bad_auth","message":"failed at {}/private Authorization: Bearer top-secret","token":"top-secret","cookie":"session=top-secret","ignored":"{}"}}"#,
        home.display(),
        "x".repeat(4_000)
    );
    let redacted = redact_diagnostic(&raw);
    let detail = redacted
        .copyable_detail
        .expect("safe allowlisted detail remains copyable");
    assert!(!detail.contains("top-secret"));
    assert!(!detail.contains(&home.display().to_string()));
    assert!(!detail.contains("\"ignored\""));
    assert!(detail.contains("[REDACTED]"));
    assert!(detail.len() <= 2_051);
    assert_eq!(redacted.fingerprint.0.len(), 64);
}

#[test]
fn primary_record_has_no_raw_payload_field_or_debug_leak() {
    let vault = DiagnosticVault::default();
    let raw = "unknown catastrophic detail with secret token=do-not-leak";
    let record = classify_provider_failure(&context(None), raw, &vault);
    let primary = format!("{record:?}");
    assert_eq!(record.failure.code, AiFailureCode::Unknown);
    assert!(!primary.contains(raw));
    assert!(!primary.contains("do-not-leak"));
    let descriptor = record.failure.diagnostic.expect("diagnostic reference");
    assert!(vault.get(&descriptor.id).is_some());
}
