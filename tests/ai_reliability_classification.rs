use script_kit_gpui::ai::reliability::{
    classify_provider_failure, redact_diagnostic, DiagnosticVault, FailureContext,
};
use serde::Deserialize;
use sk_protocol::ai_reliability::{AiFailureCode, ProtocolComponent};

const IMAGE_1: &str = include_str!("fixtures/ai-reliability/image-1-codex-client-too-old.json");
const IMAGE_2: &str = include_str!("fixtures/ai-reliability/image-2-quick-ai-search-budget.json");
const FAILURE_MATRIX: &str = include_str!("fixtures/ai-reliability/failure-matrix.json");
const SECRET_BEARING_ERROR: &str =
    include_str!("fixtures/ai-reliability/secret-bearing-error.json");

#[derive(Debug, Deserialize)]
struct MatrixFixture {
    name: String,
    status: Option<u16>,
    raw: String,
    expected: String,
}

fn context(status: Option<u16>) -> FailureContext {
    FailureContext {
        provider: Some("openai-codex".to_string()),
        model: Some("gpt-5.6-sol".to_string()),
        component: ProtocolComponent::Codex,
        http_status: status,
        retry_after_ms: None,
    }
}

#[test]
fn screenshot_fixtures_have_exact_stable_categories() {
    let vault = DiagnosticVault::default();

    assert_eq!(
        classify_provider_failure(&context(Some(400)), IMAGE_1, &vault)
            .failure
            .code,
        AiFailureCode::ClientTooOld
    );
    assert_eq!(
        classify_provider_failure(&context(None), IMAGE_2, &vault)
            .failure
            .code,
        AiFailureCode::QuickAiSearchBudgetExceeded
    );
}

#[test]
fn every_failure_fixture_is_explicitly_classified() {
    let fixtures: Vec<MatrixFixture> =
        serde_json::from_str(FAILURE_MATRIX).expect("valid failure matrix");
    let vault = DiagnosticVault::default();
    assert!(!fixtures.is_empty());
    for fixture in fixtures {
        let actual = classify_provider_failure(&context(fixture.status), &fixture.raw, &vault)
            .failure
            .code;
        assert_eq!(
            format!("{actual:?}"),
            fixture.expected,
            "fixture {}",
            fixture.name
        );
    }
}

#[test]
fn secret_fixture_has_zero_redaction_leaks() {
    let redacted = redact_diagnostic(SECRET_BEARING_ERROR);
    let safe = redacted.copyable_detail.expect("copyable safe diagnostic");
    for leak in [
        "test-fixture-token",
        "test-fixture-key",
        "/Users/tester",
        "ignored_raw_body",
    ] {
        assert!(!safe.contains(leak), "redaction leak: {leak}");
    }
    assert!(safe.contains("\"status\":401"));
    assert!(safe.contains("\"code\":\"invalid_auth\""));
}
