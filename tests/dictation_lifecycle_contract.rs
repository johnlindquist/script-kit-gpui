use script_kit_gpui::dictation::{
    resolve_delivery_target_request, resolve_final_or_partial_transcript,
    DictationDeliveryTargetResolution, DictationTarget,
};

#[test]
fn final_transcript_wins_over_partial_transcript() {
    let resolution = resolve_final_or_partial_transcript("final text", Some("partial text"));

    assert_eq!(resolution.transcript.as_deref(), Some("final text"));
    assert!(!resolution.used_partial_fallback);
    assert_eq!(resolution.final_len, "final text".len());
    assert_eq!(resolution.partial_len, Some("partial text".len()));
}

#[test]
fn partial_transcript_is_used_when_final_is_empty() {
    let resolution = resolve_final_or_partial_transcript("   ", Some("partial text"));

    assert_eq!(resolution.transcript.as_deref(), Some("partial text"));
    assert!(resolution.used_partial_fallback);
    assert_eq!(resolution.final_len, 3);
    assert_eq!(resolution.partial_len, Some("partial text".len()));
}

#[test]
fn no_transcript_is_returned_when_final_and_partial_are_empty() {
    let resolution = resolve_final_or_partial_transcript("", Some("  "));

    assert_eq!(resolution.transcript, None);
    assert!(!resolution.used_partial_fallback);
    assert_eq!(resolution.final_len, 0);
    assert_eq!(resolution.partial_len, Some(2));
}

#[test]
fn explicit_valid_target_resolves_without_fallback() {
    let result = resolve_delivery_target_request(
        Some("mainWindowFilter"),
        Some(DictationTarget::NotesEditor),
        7,
    );

    assert!(matches!(
        result,
        DictationDeliveryTargetResolution::Deliver {
            target: DictationTarget::MainWindowFilter,
            ..
        }
    ));
}

#[test]
fn explicit_invalid_target_refuses_instead_of_fallback() {
    let result = resolve_delivery_target_request(
        Some("__missing_target__"),
        Some(DictationTarget::NotesEditor),
        7,
    );

    assert!(matches!(
        result,
        DictationDeliveryTargetResolution::Refuse(_)
    ));
}

#[test]
fn implicit_target_uses_active_session() {
    let result = resolve_delivery_target_request(None, Some(DictationTarget::AiChatComposer), 7);

    assert!(matches!(
        result,
        DictationDeliveryTargetResolution::Deliver {
            target: DictationTarget::AiChatComposer,
            ..
        }
    ));
}

#[test]
fn implicit_target_refuses_when_no_active_session() {
    let result = resolve_delivery_target_request(None, None, 7);

    assert!(matches!(
        result,
        DictationDeliveryTargetResolution::Refuse(_)
    ));
}
