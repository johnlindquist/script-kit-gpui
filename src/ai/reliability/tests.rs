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
fn private_ai_reliability_fingerprints_are_keyed_before_diagnostics_leave_the_vault() {
    use sha2::{Digest as _, Sha256};

    let raw = "private provider password and exact user question";
    let public_sha = format!("{:x}", Sha256::digest(raw.as_bytes()));
    let first = redact_diagnostic(raw);
    let repeated = redact_diagnostic(raw);

    assert_eq!(first.fingerprint.0, repeated.fingerprint.0);
    assert_ne!(first.fingerprint.0, public_sha);
    assert_eq!(
        first.fingerprint.0,
        crate::logging::log_private_user_value(raw).sha256
    );
    assert_eq!(
        super::devtools::redacted_fingerprint(raw),
        first.fingerprint.0
    );
}

#[test]
fn redactor_suppresses_unallowlisted_json_without_falling_back_to_private_payloads() {
    let private_payloads = [
        serde_json::json!({
            "prompt": "private customer conversation",
            "password": "private-password",
            "api_key": "sk-private-key"
        })
        .to_string(),
        serde_json::json!([
            { "prompt": "private customer conversation", "secret": "private-secret" }
        ])
        .to_string(),
        serde_json::json!({
            "message": { "prompt": "private customer conversation" }
        })
        .to_string(),
    ];

    for raw in private_payloads {
        let redacted = redact_diagnostic(&raw);
        assert!(redacted.suppressed);
        assert!(redacted.copyable_detail.is_none());
        assert_eq!(redacted.fingerprint.0.len(), 64);
    }
}

#[test]
fn redactor_masks_passwords_and_bare_bearer_credentials_in_provider_prose() {
    let raw = "Sign in required password=private-password passphrase=private-phrase Bearer private-bearer sk-proj-private-key-material gsk_private_groq_token";
    let detail = redact_diagnostic(raw)
        .copyable_detail
        .expect("the safe recovery reason remains available");

    assert!(detail.contains("Sign in required"));
    assert!(!detail.contains("private-password"));
    assert!(!detail.contains("private-phrase"));
    assert!(!detail.contains("private-bearer"));
    assert!(!detail.contains("sk-proj-private-key-material"));
    assert!(!detail.contains("gsk_private_groq_token"));
}

#[test]
fn redactor_removes_complete_multiline_private_keys() {
    let raw = "Key rejected -----BEGIN PRIVATE KEY-----\nprivate-secret-material\n-----END PRIVATE KEY----- while signing";
    let detail = redact_diagnostic(raw)
        .copyable_detail
        .expect("the safe signing context remains available");

    assert!(detail.contains("Key rejected"));
    assert!(detail.contains("while signing"));
    assert!(!detail.contains("BEGIN PRIVATE KEY"));
    assert!(!detail.contains("private-secret-material"));
}

#[test]
fn redactor_preserves_product_crate_names_that_are_not_provider_tokens() {
    let raw = "Check crates/sk-protocol/src/command_contract.rs before running sk-protocol tests";
    let detail = redact_diagnostic(raw)
        .copyable_detail
        .expect("product crate names contain no provider credentials");

    assert_eq!(detail, raw);
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

/// Focused check for `rules/AI_RELIABILITY.md` Rules 1-3 (S13).
///
/// Every typed classifier takes a FACT the runtime stated, so none of them may
/// return `Unknown` — and the copy each one produces must be safe copy that,
/// fed back through the free-text classifier, does NOT reproduce the original
/// classification. That second half is the whole point: it is what makes a
/// string round-trip a silent downgrade rather than a harmless detour.
///
/// A failure here means someone routed a fact through prose again. Read the
/// rules file before changing the expectations.
#[test]
fn typed_classifiers_never_return_unknown_and_their_copy_is_not_evidence() {
    let vault = DiagnosticVault::default();
    let component = ProtocolComponent::Pi;
    let cases: Vec<(&str, AppFailureRecord, AiFailureCode)> = vec![
        (
            "SetupRequired",
            classify_setup_required(
                &FailureContext {
                    component,
                    ..FailureContext::default()
                },
                "login required",
                &["browser".to_string()],
                &vault,
            ),
            AiFailureCode::AuthenticationMissing,
        ),
        (
            "spawn failed",
            classify_spawn_failure(
                &FailureContext {
                    component,
                    ..FailureContext::default()
                },
                "No such file or directory (os error 2)",
                &vault,
            ),
            AiFailureCode::SpawnFailed,
        ),
        (
            "runtime closed",
            classify_runtime_closed(
                &FailureContext {
                    component,
                    ..FailureContext::default()
                },
                "Broken pipe (os error 32)",
                &vault,
            ),
            AiFailureCode::RuntimeClosed,
        ),
        (
            "context unavailable",
            classify_context_unavailable(
                &FailureContext {
                    component,
                    ..FailureContext::default()
                },
                "SAFE001_RAW_CONTEXT_ERROR_CANARY",
                &vault,
            ),
            AiFailureCode::ContextUnavailable,
        ),
        (
            "child exited",
            classify_process_failure(
                &FailureContext {
                    component,
                    ..FailureContext::default()
                },
                ProcessFailureFacts::ChildExited {
                    exit_code: Some(3),
                    signal: None,
                },
                &vault,
            ),
            AiFailureCode::ChildExited,
        ),
    ];

    for (label, record, expected) in cases {
        assert_eq!(
            record.failure.code, expected,
            "{label}: a stated fact must classify to its own code"
        );

        // Rule 3: the cause survives, but only behind the vault.
        let descriptor = record
            .failure
            .diagnostic
            .as_ref()
            .unwrap_or_else(|| panic!("{label}: the cause must reach the diagnostic vault"));
        assert!(
            vault.get(&descriptor.id).is_some(),
            "{label}: the vault must be able to produce the cause for Copy Details"
        );
        let debug = format!("{record:?}");
        assert!(
            !debug.contains("os error")
                && !debug.contains("login required")
                && !debug.contains("SAFE001_RAW_CONTEXT_ERROR_CANARY"),
            "{label}: the raw cause must not survive in the record itself"
        );

        // Rule 2: round-tripping the safe copy loses the classification. This
        // is why the record must be carried, never re-derived.
        let round_tripped = classify_provider_failure(
            &FailureContext {
                component,
                ..FailureContext::default()
            },
            record.primary_message(),
            &vault,
        );
        assert_eq!(
            round_tripped.failure.code,
            AiFailureCode::Unknown,
            "{label}: safe copy is not classifiable evidence — carry the record instead"
        );
    }
}
