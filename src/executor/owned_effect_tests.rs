use crate::ai::providers::{AiProvider, OpenAiProvider};
use crate::platform::accessibility::{mutation, FocusedTextSessionId, TextMutationOptions};
use crate::runtime_policy::{install_owned_evaluation, owned_evaluation, OwnedEvaluationPolicy};

#[test]
fn owned_effect_boundaries_refuse_before_resources() {
    const CHILD: &str = "SCRIPT_KIT_OWNED_EFFECT_BOUNDARY_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "executor::owned_effect_tests::owned_effect_boundaries_refuse_before_resources",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .status()
            .expect("launch isolated policy test process");
        assert!(status.success());
        return;
    }

    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    install_owned_evaluation(
        OwnedEvaluationPolicy::new(&root, "boundary-test".into(), "generation-1".into()).unwrap(),
    )
    .unwrap();

    // Empty operations are deliberately safe even if a guard regresses. Owned
    // execution still refuses them rather than manufacturing a success receipt.
    let injector = crate::text_injector::TextInjector::new();
    assert_eq!(
        injector.delete_chars(0).unwrap_err().to_string(),
        "native_input_forbidden"
    );
    assert_eq!(
        injector.paste_text("").unwrap_err().to_string(),
        "system_clipboard_forbidden"
    );
    assert_eq!(
        crate::platform::accessibility::clipboard::write_plain_text_to_pasteboard("")
            .unwrap_err()
            .to_string(),
        "system_clipboard_forbidden"
    );

    // The intentionally invalid transport URL cannot contact a provider, even
    // on regression. The observable error must come from policy, not transport.
    let provider = OpenAiProvider::with_base_url("fixture-only-key", "not-a-url");
    assert_eq!(
        provider
            .send_message(&[], "fixture-model")
            .unwrap_err()
            .to_string(),
        "provider_forbidden"
    );

    let session = FocusedTextSessionId("owned-fixture-text".into());
    mutation::register_in_memory_focused_text_target(&session, "before");
    let policy = owned_evaluation().unwrap();
    let completed_before = policy.completed_fixture_effect_count();
    let changed =
        mutation::replace_focused_text(session.clone(), "after", TextMutationOptions::default())
            .unwrap();
    assert!(changed.changed_text);
    assert!(!changed.copied_to_clipboard);
    assert_eq!(
        mutation::in_memory_focused_text(&session).as_deref(),
        Some("after")
    );
    assert_eq!(
        policy.completed_fixture_effect_count(),
        completed_before + 1
    );

    let unchanged =
        mutation::replace_focused_text(session, "after", TextMutationOptions::default()).unwrap();
    assert!(!unchanged.changed_text);
    assert_eq!(
        policy.completed_fixture_effect_count(),
        completed_before + 1
    );

    // An unregistered target must not fall through to AX, selection paste or
    // native focus. No fixture result is synthesized for missing targets.
    let refusal = mutation::append_focused_text(
        FocusedTextSessionId("not-registered".into()),
        "text",
        TextMutationOptions::default(),
    )
    .unwrap_err();
    assert_eq!(refusal.to_string(), "native_input_forbidden");
    assert_eq!(
        policy.completed_fixture_effect_count(),
        completed_before + 1
    );
    assert!(policy.refused_effect_count() >= 5);
}
