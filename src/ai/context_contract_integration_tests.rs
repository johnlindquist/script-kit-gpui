use crate::ai::context_contract::ContextAttachmentKind;
use crate::ai::context_mentions::parse_context_mentions;
use crate::ai::message_parts::{
    merge_context_parts, prepare_user_message_with_receipt, PreparedMessageDecision,
};

#[test]
fn explicit_context_surfaces_share_one_contract_end_to_end() {
    crate::context_snapshot::enable_deterministic_context_capture();
    // Step 1: Parse @mentions through the production parser (which delegates
    // to the canonical context_contract module).
    let parsed = parse_context_mentions("@snapshot\n@browser\n\nPlease summarize what matters.");

    assert_eq!(
        parsed.cleaned_content, "Please summarize what matters.",
        "mention lines should be stripped from cleaned content"
    );
    assert_eq!(
        parsed.parts.len(),
        2,
        "should parse @snapshot and @browser; got: {:?}",
        parsed.parts
    );

    // Step 2: Simulate pending context chips added via UI actions.
    let pending = vec![
        ContextAttachmentKind::Current.part(),
        ContextAttachmentKind::Diagnostics.part(),
    ];

    // Step 3: Merge — duplicate current-context (from both mention and pending)
    // should be deduplicated, preserving first-seen (mention) order.
    let merged = merge_context_parts(&parsed.parts, &pending);

    assert_eq!(
        merged.len(),
        3,
        "duplicate current context should be removed while preserving first-seen order; got: {:?}",
        merged.iter().map(|p| p.label()).collect::<Vec<_>>()
    );

    // Step 4: Prepare through the full pipeline with empty script/scriptlet
    // lists — kit://context URIs resolve via the MCP resource handler which
    // works without scripts.
    let receipt = prepare_user_message_with_receipt(&parsed.cleaned_content, &merged, &[], &[]);

    assert_eq!(
        receipt.decision,
        PreparedMessageDecision::Ready,
        "all kit://context URIs should resolve; decision was {:?}",
        receipt.decision
    );
    assert_eq!(receipt.context.attempted, 3);
    assert_eq!(receipt.context.resolved, 3);
    assert_eq!(
        receipt.context.failed, 0,
        "unexpected failure count: {}",
        receipt.context.failed
    );

    // Verify the final content contains all three expected context URIs
    // plus the cleaned user text at the end.
    assert!(
        receipt
            .final_user_content
            .contains("kit://context?profile=minimal"),
        "final content should contain minimal context URI"
    );
    assert!(
        receipt.final_user_content.contains(
            "kit://context?selectedText=0&frontmostApp=0&menuBar=0&browserUrl=1&focusedWindow=0"
        ),
        "final content should contain browser context URI"
    );
    assert!(
        receipt
            .final_user_content
            .contains("kit://context?diagnostics=1"),
        "final content should contain diagnostics URI"
    );
    assert!(
        receipt
            .final_user_content
            .ends_with("Please summarize what matters."),
        "final content should end with user text; got: ...{}",
        &receipt.final_user_content[receipt.final_user_content.len().saturating_sub(60)..]
    );
}
