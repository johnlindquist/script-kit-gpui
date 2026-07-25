use super::*;
use sk_protocol::ai_reliability::{
    AiFailureCode, AiFailureKind, CapabilityFailure, ClientKind, ProtocolComponent, ProtocolFailure,
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
fn codex_named_upgrade_error_wins_over_pi_transport_component() {
    let vault = DiagnosticVault::default();
    let mut pi_context = context(Some(400));
    pi_context.component = ProtocolComponent::Pi;
    let record = classify_provider_failure(
        &pi_context,
        r#"{"type":"error","status":400,"error":{"type":"invalid_request_error","message":"The 'gpt-5.6-sol' model requires a newer version of Codex. Please upgrade to the latest app or CLI and try again."}}"#,
        &vault,
    );
    assert!(matches!(
        record.failure.kind,
        AiFailureKind::Capability(CapabilityFailure::ClientTooOld {
            client: ClientKind::Codex,
            ..
        })
    ));
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
fn output_schema_rejection_is_configuration_not_missing_authentication() {
    let vault = DiagnosticVault::default();
    let record = classify_provider_failure(
        &context(Some(400)),
        "Invalid schema for response_format: output schema uses an unsupported keyword; no API key fallback is available",
        &vault,
    );
    assert_eq!(
        record.failure.code,
        AiFailureCode::InvalidConfiguration,
        "precise schema evidence must win over broad authentication wording"
    );
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

/// Provider stderr must not decide what Quick AI tells the user to do.
///
/// Quick AI used to hand `<our code>\n<raw codex stderr>` to the free-text
/// provider classifier. That classifier pattern-matches English, so a blocked
/// shell command whose stderr happened to contain "unauthorized" produced a
/// "sign in to continue" card, and one containing "operation not permitted"
/// produced a permission card — neither of which had anything to do with the
/// actual failure. The user was sent to fix something that was not broken.
#[test]
fn quick_ai_codes_outrank_misleading_words_in_provider_stderr() {
    let vault = DiagnosticVault::default();

    // The exact shape captured in production: a forbidden shell command whose
    // stderr contains words the text classifier treats as auth and permission
    // evidence.
    let forbidden = quick_ai_turn_failure(
        &context(None),
        "quick_ai_codex_forbidden_item:command_execution:item_0",
        "quick_ai_codex_forbidden_item:command_execution:item_0\n\
         unauthorized: operation not permitted, no api key found",
        &vault,
    )
    .expect("a quick_ai_* code must classify without consulting stderr");
    assert_eq!(forbidden.failure.code, AiFailureCode::ToolDenied);

    // Same stderr, different Quick AI code: still classified from the code.
    let schema = quick_ai_turn_failure(
        &context(None),
        "quick_ai_output_schema_source_invalid",
        "quick_ai_output_schema_source_invalid\nunauthorized",
        &vault,
    )
    .expect("schema violations are protocol failures");
    assert_eq!(
        schema.failure.code,
        AiFailureCode::ProtocolMalformedResponse
    );

    let truncated = quick_ai_turn_failure(
        &context(None),
        "quick_ai_codex_eof_without_terminal:1",
        "quick_ai_codex_eof_without_terminal:1",
        &vault,
    )
    .expect("a truncated stream is a missing terminal");
    assert_eq!(
        truncated.failure.code,
        AiFailureCode::ProtocolMissingTerminal
    );

    // A code we do not own still falls through to the text classifier, so real
    // provider auth failures keep their sign-in affordance.
    assert!(
        quick_ai_turn_failure(&context(None), "unauthorized", "unauthorized", &vault).is_none()
    );
    assert_eq!(
        classify_provider_failure(&context(None), "unauthorized", &vault)
            .failure
            .code,
        AiFailureCode::AuthenticationMissing
    );
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
