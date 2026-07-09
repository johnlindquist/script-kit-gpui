use std::fs;

#[test]
fn attachment_portal_layout_uses_dedicated_split_nodes() {
    let source = fs::read_to_string("src/app_layout/build_layout_info.rs")
        .expect("build_layout_info source should be readable");

    for needle in [
        "AttachmentPortalHeader",
        "AttachmentPortalSearch",
        "AttachmentPortalContent",
        "AttachmentPortalList",
        "AttachmentPortalPreview",
        "AttachmentPortalRow",
        "AttachmentPortalRunButton",
        "AttachmentPortalActionsButton",
    ] {
        assert!(
            source.contains(needle),
            "Attachment Portal layout receipt is missing `{needle}`"
        );
    }
}

#[test]
fn attachment_portal_split_nodes_use_liquid_glass_panel_radius() {
    let source = fs::read_to_string("src/app_layout/build_layout_info.rs")
        .expect("build_layout_info source should be readable");

    for node in [
        "AttachmentPortalContent",
        "AttachmentPortalList",
        "AttachmentPortalPreview",
    ] {
        let start = source
            .find(&format!("LayoutComponentInfo::new(\"{node}\""))
            .unwrap_or_else(|| panic!("{node} layout node should exist"));
        let node_source = &source[start..];
        let end = node_source
            .find(".with_visual_token")
            .unwrap_or_else(|| panic!("{node} should declare visual metadata"));
        assert!(
            node_source[..end].contains("Some(chrome_tokens::LIQUID_GLASS_PANEL_RADIUS_PX)"),
            "{node} must use the shared Liquid Glass panel radius token"
        );
    }
}

#[test]
fn attachment_portal_proof_matrix_uses_attachment_portal_receipt() {
    let source = fs::read_to_string("scripts/devtools/liquid-glass-proof.ts")
        .expect("liquid-glass proof source should be readable");

    assert!(
        source.contains("AttachmentPortalBrowser")
            && source.contains("window-priority-attachment-portal-current-layout.json"),
        "Liquid Glass proof matrix must attach the Attachment Portal visual audit receipt"
    );
}

#[test]
fn proof_matrix_rejects_nested_near_black_screenshot_receipts() {
    let source = fs::read_to_string("scripts/devtools/liquid-glass-proof.ts")
        .expect("liquid-glass proof source should be readable");

    assert!(
        source.contains("screenshotReceipt.contentAudit"),
        "proof matrix must inspect verify-shot's nested screenshotReceipt content audit"
    );
    assert!(
        source.contains("nonBlackRatio < 0.01"),
        "proof matrix must reject screenshots that are mathematically near black"
    );
    assert!(
        source.contains("ignored screenshot")
            && source.contains("below 0.01 usable-capture threshold"),
        "proof matrix must record why an existing screenshot was excluded"
    );
}
