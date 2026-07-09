//! Source-level contract for Liquid Glass guideline assertion buckets.

use std::fs;

const TAHOE_NATIVE_BASELINE_RECEIPT: &str = r#"
{
  "textFields": [
    {
      "controlSize" : "regular",
      "contentHorizontalInsetPt" : 9,
      "contentVerticalInsetPt" : 3,
      "intrinsicHeightPt" : 22
    }
  ],
  "glassEffectView": {
    "defaultCornerRadiusPt" : 8
  },
  "osVersion": "26.5"
}
"#;

const TAHOE_WINDOW_MASK_BASELINE_RECEIPT: &str = r#"
{
  "styles": [
    {
      "style": "titledStandardWindow",
      "cornerRadiusPt" : 15
    }
  ],
  "ourWindowRadiusTokenPt" : 22
}
"#;

fn layout_component_source<'a>(layout: &'a str, node: &str) -> &'a str {
    let inline = format!("LayoutComponentInfo::new(\"{node}\"");
    let multiline = format!("LayoutComponentInfo::new(\n                    \"{node}\"");
    let start = layout
        .find(&inline)
        .or_else(|| layout.find(&multiline))
        .unwrap_or_else(|| panic!("{node} layout node should exist"));
    &layout[start..]
}

#[test]
fn layout_visual_audit_emits_guideline_assertion_buckets() {
    let layout =
        fs::read_to_string("scripts/devtools/layout.ts").expect("failed to read layout.ts");
    for needle in [
        "guidelineAssertions",
        "appleDocumented",
        "projectLocal",
        "buttonCenterDistance",
        "macosMinimumHitSize",
        "macosMinimumVisualSize",
        "materialLayering",
        "colorAdaptivity",
        "cornerRadiusTokens",
        "paddingTokens",
        "spacingTokens",
    ] {
        assert!(layout.contains(needle), "layout.ts must expose {needle}");
    }
}

#[test]
fn secondary_target_layout_receipts_synthesize_window_backdrop_metadata() {
    let layout =
        fs::read_to_string("scripts/devtools/layout.ts").expect("failed to read layout.ts");

    for needle in [
        "ensureRootWindowBackdropNode",
        "windowBackdrop",
        "nativeWindowBackdrop",
        "window.backdrop",
        "contentNativeMaterialNodes",
        "glassLayerViolations",
        "rawLayout",
    ] {
        assert!(
            layout.contains(needle),
            "layout.ts must expose secondary-window backdrop proof marker {needle}"
        );
    }
}

/// The `appleDocumented` bucket must encode Apple's documented numeric
/// guidelines as real per-node deviation math, NOT the previous
/// `exactAppleRadiusConstants: null` placeholder. Apple publishes formulas
/// (capsule = height/2, concentric child = parent - inset) and a few hard
/// spacing constants; the engine compares measured nodes against those.
#[test]
fn apple_documented_bucket_emits_numeric_conformance_not_null_placeholder() {
    let layout =
        fs::read_to_string("scripts/devtools/layout.ts").expect("failed to read layout.ts");

    assert!(
        !layout.contains("exactAppleRadiusConstants: null"),
        "appleDocumented must not ship a null radius-constants placeholder; encode Apple's concentric/capsule formulas + deviation math instead"
    );
    assert!(
        layout.contains("appleGuidelineConformance")
            && layout.contains("apple-guideline-constants"),
        "layout.ts must wire the Apple-guideline conformance engine"
    );
    for needle in ["cornerGeometry", "conformanceScore", "backingScaleFactor"] {
        assert!(
            layout.contains(needle),
            "appleDocumented conformance must expose {needle}"
        );
    }
}

/// The Apple-guideline constants table must be provenance-tagged (so each value
/// is defensible) and must encode Apple's actually-documented formulas/constants
/// — capsule radius = height/2, concentric child = parent - inset, the 60pt
/// regular-button center distance and 16pt minimum gap, and the ~12pt bezel
/// padding heuristic — with copyright-safe paraphrased summaries (no copied
/// Apple prose).
#[test]
fn apple_guideline_constants_are_provenance_tagged_and_documented() {
    let constants = fs::read_to_string("scripts/devtools/apple-guideline-constants.ts")
        .expect("failed to read apple-guideline-constants.ts");

    for field in [
        "confidence",
        "normativeStrength",
        "copyrightSafeSummary",
        "appleReference",
        "GuidelineConfidence",
    ] {
        assert!(
            constants.contains(field),
            "guideline constants must carry provenance field {field}"
        );
    }
    for metric in [
        "shape.capsule.radius",
        "shape.concentric.childRadius",
        "layout.regularButton.centerDistance",
        "layout.regularButton.minimumGap",
        "layout.bezelElement.padding",
        "macos.window.toolbarRadius.nativeBaseline",
    ] {
        assert!(
            constants.contains(metric),
            "guideline constants must define Apple metric {metric}"
        );
    }
    // Apple's documented formulas / hard numbers.
    assert!(
        constants.contains("radiusPt = heightPt / 2"),
        "must encode Apple's capsule radius = height/2 formula"
    );
    assert!(
        constants.contains("childRadiusPt = max(0, parentRadiusPt - separationPt)"),
        "must encode Apple's concentric radius formula"
    );
    assert!(
        constants.contains("minPt: 60") && constants.contains("minPt: 16"),
        "must encode Apple's 60pt center-distance and 16pt minimum-gap constants"
    );
    // Deviation classification vocabulary the proof matrix and tests rely on.
    for token in [
        "withinBand",
        "nearBand",
        "outOfBand",
        "unmeasured",
        "deltaPt",
    ] {
        assert!(
            constants.contains(token),
            "conformance engine must classify deviations with {token}"
        );
    }
}

/// Slice 2 (Oracle session `tahoe-apple-guideline-metrics`): the conformance
/// engine must compare against REAL macOS 26 control geometry, captured by a
/// re-runnable Swift probe and persisted as a receipt — not guessed soft bands.
/// This pins the probe, the receipt's measured numbers, and the measuredNative
/// metrics that cite them so they cannot drift apart silently.
#[test]
fn measured_native_baselines_are_probe_backed_and_pinned() {
    let probe = fs::read_to_string("scripts/devtools/tahoe_native_baseline.swift")
        .expect("native baseline swift probe must exist");
    assert!(
        probe.contains("NSTextField") && probe.contains("NSGlassEffectView"),
        "probe must measure native NSTextField + NSGlassEffectView geometry"
    );
    assert!(
        probe.contains("contentHorizontalInsetPt") && probe.contains("defaultCornerRadiusPt"),
        "probe must emit the inset + glass radius fields the engine pins against"
    );

    let receipt = TAHOE_NATIVE_BASELINE_RECEIPT;
    for needle in [
        "\"controlSize\" : \"regular\"",
        "\"contentHorizontalInsetPt\" : 9",
        "\"contentVerticalInsetPt\" : 3",
        "\"defaultCornerRadiusPt\" : 8",
        "26.5",
    ] {
        assert!(
            receipt.contains(needle),
            "native baseline receipt must record measured value {needle}"
        );
    }

    let constants = fs::read_to_string("scripts/devtools/apple-guideline-constants.ts")
        .expect("failed to read apple-guideline-constants.ts");
    for metric in [
        "control.searchField.textInset.horizontal",
        "control.searchField.textInset.vertical",
        "control.regular.height",
        "macos.glassEffectView.defaultRadius",
    ] {
        assert!(
            constants.contains(metric),
            "constants must encode measured-native metric {metric}"
        );
    }
    assert!(
        constants.contains("nativeMeasurement")
            && constants.contains("tahoe_native_baseline.swift")
            && constants.contains("tahoe-native-baseline.json"),
        "measuredNative metrics must cite the swift probe + receipt as provenance"
    );
}

/// Slice 5 (Oracle session `tahoe-apple-guideline-metrics`): the proven input
/// padding fix. The main-launcher SearchInput now both RENDERS and EMITS the
/// configured main-input inset (SEARCH_INPUT_TEXT_INSET_X_PX = 16pt) instead
/// of flush 0pt text. The conformance engine therefore classifies it withinBand.
/// This pins the token, the render-layer left padding, and the layout emission so
/// the audit can never claim padding while the UI still renders 0pt (no fake-green).
#[test]
fn search_input_renders_and_emits_configured_content_inset() {
    let tokens =
        fs::read_to_string("src/ui/chrome/tokens.rs").expect("failed to read chrome tokens.rs");
    assert!(
        tokens.contains("pub const SEARCH_INPUT_TEXT_INSET_X_PX: f32 = 16.0;"),
        "the search-input inset token must match the saved main input padding X default"
    );

    let style = fs::read_to_string("src/protocol/types/grid_layout.rs")
        .expect("failed to read grid_layout.rs");
    assert!(
        style.contains("pub content_insets: Option<BoxModelSides>")
            && style.contains("fn with_content_insets"),
        "LayoutVisualStyle must carry an emittable content_insets field + builder"
    );

    // The REAL render layer must apply the padding through the shared main-view
    // input shell, not just emit it in the audit, so the audit cannot fake-green.
    let render = fs::read_to_string("src/components/main_view_chrome.rs")
        .expect("failed to read main_view_chrome.rs");
    assert!(
        render.contains(
            "pub(crate) fn main_view_input_text_inset_left(def: MainMenuThemeDef) -> f32"
        ) && render.contains("def.search.text_inset_x")
            && render.contains(".pl(px(text_inset_left))"),
        "the search input must render its configured left padding, not only emit it in the audit"
    );

    let theme = fs::read_to_string("src/designs/core/main_menu_theme.rs")
        .expect("failed to read main_menu_theme.rs");
    assert!(
        theme.contains("text_inset_x: crate::ui::chrome::SEARCH_INPUT_TEXT_INSET_X_PX"),
        "main menu search token must use the shared input inset token"
    );

    let layout = fs::read_to_string("src/app_layout/build_layout_info.rs")
        .expect("failed to read build_layout_info.rs");
    assert!(
        layout.contains(
            "crate::components::main_view_chrome::main_view_input_text_inset_left(menu_def)"
        ),
        "MainViewInput layout must source its left inset from the shared main_view_chrome helper"
    );
    let node = layout_component_source(&layout, "MainViewInput");
    let end = node
        .find(".with_visual_token(\"chrome.mainViewInput\")")
        .expect("MainViewInput should declare its visual token");
    let node = &node[..end];
    assert!(
        node.contains(".with_content_insets(")
            && node.contains("input_text_inset_left")
            && node.contains("search.text_inset_x"),
        "MainViewInput must emit the configured content inset, matching the render layer"
    );
    assert!(
        !node.contains(
            "0.0,\n                    crate::panel::CURSOR_MARGIN_Y,\n                    0.0,"
        ),
        "MainViewInput must no longer emit the pre-fix flush 0.0pt horizontal inset"
    );
}

/// Slice 3b (Oracle session `tahoe-apple-guideline-metrics`): the main-launcher
/// must EMIT a footer rail node carrying its real inter-item gap (boxModel.gap =
/// footer_metrics.item_gap_px) so the conformance engine can MEASURE the user's
/// "footer lacks padding" concern (6pt observed vs the soft ~12pt floor). The
/// metric is SOFT — footer hint chips are not regular buttons — so we must not
/// overstate it as a hard Apple violation.
#[test]
fn main_footer_emits_measured_item_gap_for_soft_conformance() {
    let layout = fs::read_to_string("src/app_layout/build_layout_info.rs")
        .expect("failed to read build_layout_info.rs");
    let node = layout_component_source(&layout, "MainViewFooter");
    let end = node
        .find(".with_explanation(")
        .expect("MainViewFooter should declare an explanation");
    let node = &node[..end];
    assert!(
        node.contains(".with_gap(footer_metrics.item_gap_px)"),
        "MainViewFooter must emit its real inter-item gap as boxModel.gap"
    );
    assert!(
        node.contains(".with_content_insets("),
        "MainViewFooter must emit its content insets (side padding)"
    );

    let constants = fs::read_to_string("scripts/devtools/apple-guideline-constants.ts")
        .expect("failed to read apple-guideline-constants.ts");
    assert!(
        constants.contains("layout.footer.itemGap"),
        "constants must define the footer item-gap metric"
    );
    // Must be SOFT/derived, not a hard documented Apple constant.
    let metric_block = constants
        .split("id: \"layout.footer.itemGap\"")
        .nth(1)
        .and_then(|tail| tail.get(..400))
        .unwrap_or("");
    assert!(
        metric_block.contains("normativeStrength: \"soft\"")
            && metric_block.contains("confidence: \"derived\""),
        "footer item-gap metric must be soft/derived (footer hints are not regular buttons)"
    );
    assert!(
        constants.contains("footerSpacingDeviations")
            && constants.contains("...footerSpacingDeviations(nodes, scale)"),
        "footerSpacingDeviations must be wired into the conformance rollup"
    );
}

/// Slice 3c (Oracle session `tahoe-apple-guideline-metrics`): the window-radius
/// concern (#1) must be measured against a REAL native Tahoe window mask, not a
/// placeholder probe. The native window-mask probe measured titled/glass windows
/// at 15pt; our token is 22pt, so the concern is REFUTED (we are rounder than
/// native). This pins the probe, the receipt (15pt native), and the metric
/// modeled as a soft minimum so "rounder than native" is not a failure.
#[test]
fn window_radius_is_measured_against_native_mask_baseline() {
    let probe = fs::read_to_string("scripts/devtools/tahoe_window_mask_probe.swift")
        .expect("native window-mask probe must exist");
    assert!(
        probe.contains("screencapture") && probe.contains("measureCornerRadiusPx"),
        "probe must screenshot native windows and measure their corner mask"
    );

    let receipt = TAHOE_WINDOW_MASK_BASELINE_RECEIPT;
    for needle in [
        "\"cornerRadiusPt\" : 15",
        "titledStandardWindow",
        "\"ourWindowRadiusTokenPt\" : 22",
    ] {
        assert!(
            receipt.contains(needle),
            "window-mask receipt must record measured value {needle}"
        );
    }

    let constants = fs::read_to_string("scripts/devtools/apple-guideline-constants.ts")
        .expect("failed to read apple-guideline-constants.ts");
    assert!(
        constants.contains("windowRadiusDeviations")
            && constants.contains("...windowRadiusDeviations(nodes, scale)"),
        "windowRadiusDeviations must be wired into the conformance rollup"
    );
    // The window metric must be a soft minimum (rounder-than-native is allowed),
    // and must cite the window-mask probe + receipt.
    let metric_block = constants
        .split("id: \"macos.window.toolbarRadius.nativeBaseline\"")
        .nth(1)
        .and_then(|tail| tail.get(..1100))
        .unwrap_or("");
    assert!(
        metric_block.contains("kind: \"minimum\", minPt: 15")
            && metric_block.contains("normativeStrength: \"soft\""),
        "window-radius metric must be a soft 15pt minimum (measured native floor)"
    );
    assert!(
        metric_block.contains("tahoe_window_mask_probe.swift")
            && metric_block.contains("tahoe-window-mask-baseline.json"),
        "window-radius metric must cite the window-mask probe + receipt"
    );
}

#[test]
fn layout_visual_audit_rejects_zero_radius_placeholders() {
    let layout =
        fs::read_to_string("scripts/devtools/layout.ts").expect("failed to read layout.ts");
    let proof = fs::read_to_string("scripts/devtools/liquid-glass-proof.ts")
        .expect("failed to read liquid-glass-proof.ts");
    assert!(
        layout.contains("function hasPositiveRadius"),
        "layout.ts must validate radius values, not only radius field presence"
    );
    assert!(
        layout.contains("entry > 0") && layout.contains("value > 0"),
        "layout.ts must reject zero-radius placeholders in guideline assertions"
    );
    assert!(
        layout.contains("!hasPositiveRadius(style.cornerRadius)")
            && layout.contains("!hasPositiveRadius(style.radius)"),
        "cornerRadiusTokens failures must be based on positive Liquid Glass radii"
    );
    assert!(
        !proof.contains("REQUIRED_POSITIVE_RADIUS_NODE_NAMES"),
        "proof matrix must not hide zero-radius styled nodes behind a hard-coded name whitelist"
    );
    assert!(
        !layout.contains("REQUIRED_POSITIVE_RADIUS_NODE_NAMES"),
        "layout audit must not hide zero-radius styled nodes behind a hard-coded name whitelist"
    );
}

fn snippet_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_index = source
        .find(start)
        .unwrap_or_else(|| panic!("missing start marker `{start}`"));
    let end_index = source[start_index..]
        .find(end)
        .map(|offset| start_index + offset)
        .unwrap_or_else(|| panic!("missing end marker `{end}`"));
    &source[start_index..end_index]
}

/// The corner-radius audit must decide "is this node a rounded surface?" from
/// its node TYPE, not from a hard-coded node-name list. Both audit layers
/// (`layout.ts` cornerRadiusFailures and `liquid-glass-proof.ts`
/// nodesWithMissingPositiveRadius) must share the same type-only predicate so
/// they cannot drift, and Other/text nodes (bare labels, 1px dividers) must
/// stay out of the radius-bearing set.
#[test]
fn corner_radius_audit_is_type_based_not_name_whitelisted() {
    let layout =
        fs::read_to_string("scripts/devtools/layout.ts").expect("failed to read layout.ts");
    let proof = fs::read_to_string("scripts/devtools/liquid-glass-proof.ts")
        .expect("failed to read liquid-glass-proof.ts");

    let layout_predicate = snippet_between(
        &layout,
        "const RADIUS_BEARING_NODE_TYPES",
        "function rectFrom",
    );
    let proof_predicate = snippet_between(
        &proof,
        "const RADIUS_BEARING_NODE_TYPES",
        "function nodesWithMissingPositiveRadius",
    );

    for predicate in [layout_predicate, proof_predicate] {
        assert!(
            predicate.contains("RADIUS_BEARING_NODE_TYPES.has(type)"),
            "radius-bearing classification must be type-based"
        );
        for node_type in [
            "\"area\"",
            "\"button\"",
            "\"card\"",
            "\"container\"",
            "\"header\"",
            "\"input\"",
            "\"list\"",
            "\"listitem\"",
            "\"panel\"",
            "\"prompt\"",
            "\"window\"",
        ] {
            assert!(
                predicate.contains(node_type),
                "radius-bearing type set must include {node_type}"
            );
        }
        assert!(
            !predicate.contains("\"other\"") && !predicate.contains("\"text\""),
            "Other/Text nodes must not be radius-bearing surfaces"
        );
        assert!(
            !predicate.contains("KitStore")
                && !predicate.contains("GenericFilterable")
                && !predicate.contains("REQUIRED_POSITIVE_RADIUS_NODE_NAMES"),
            "radius audit must not use surface-name carveouts"
        );
        assert!(
            !predicate.contains("/Area|Content|Panel"),
            "radius audit should not use the old name-regex heuristic"
        );
    }

    assert!(
        layout.contains("if (!isRadiusBearingNode(node)) return false;"),
        "layout.ts cornerRadiusFailures must skip only non-radius-bearing node types"
    );
}

#[test]
fn shared_prompt_layout_nodes_use_liquid_glass_radius_tokens() {
    let layout = fs::read_to_string("src/app_layout/build_layout_info.rs")
        .expect("failed to read build_layout_info.rs");

    for node in ["MainViewMain"] {
        let node_source = layout_component_source(&layout, node);
        let end = node_source
            .find(".with_visual_token")
            .unwrap_or_else(|| panic!("{node} should declare visual metadata"));
        assert!(
            node_source[..end].contains("Some(chrome_tokens::LIQUID_GLASS_PANEL_RADIUS_PX)"),
            "{node} must use the shared Liquid Glass panel radius token"
        );
        assert!(
            !node_source[..end].contains("Some(0.0)"),
            "{node} must not satisfy guideline proof with a zero-radius placeholder"
        );
    }

    for node in [
        "DivContent",
        "SelectChoices",
        "EnvPromptContent",
        "TerminalContent",
    ] {
        let start = layout
            .find(&format!("\"{node}\""))
            .unwrap_or_else(|| panic!("{node} layout node should exist"));
        let node_source = &layout[start..];
        let end = node_source
            .find("return LayoutInfo")
            .unwrap_or_else(|| panic!("{node} should return after visual metadata"));
        assert!(
            node_source[..end].contains("Some(chrome_tokens::LIQUID_GLASS_PANEL_RADIUS_PX)"),
            "{node} must use the shared Liquid Glass panel radius token"
        );
        assert!(
            !node_source[..end].contains("Some(0.0)"),
            "{node} must not satisfy guideline proof with a zero-radius placeholder"
        );
    }
}

#[test]
fn creation_feedback_content_nodes_use_liquid_glass_panel_radius() {
    let layout = fs::read_to_string("src/app_layout/build_layout_info.rs")
        .expect("failed to read build_layout_info.rs");
    let start = layout
        .find("if matches!(self.current_view, AppView::CreationFeedback")
        .expect("CreationFeedback layout branch should exist");
    let branch = &layout[start..];

    for node in [
        "CreationFeedbackIntro",
        "CreationFeedbackArtifactSection",
        "CreationFeedbackVerificationSection",
        "CreationFeedbackReceiptSection",
        "CreationFeedbackArtifactActions",
        "CreationFeedbackReceiptActions",
    ] {
        assert!(
            branch.contains(node),
            "{node} layout node should exist in the CreationFeedback branch"
        );
    }

    assert!(
        branch.matches("Some(chrome_tokens::LIQUID_GLASS_PANEL_RADIUS_PX)")
            .count()
            >= 5,
        "CreationFeedback content/action containers must use the shared Liquid Glass panel radius token"
    );
}

#[test]
fn actions_list_layout_node_uses_liquid_glass_panel_radius() {
    let dialog =
        fs::read_to_string("src/actions/dialog.rs").expect("failed to read actions/dialog.rs");

    let start = dialog
        .find("\"ActionsList\"")
        .expect("ActionsList layout node should exist");
    let node_source = &dialog[start..];
    let end = node_source
        .find(".with_visual_token")
        .expect("ActionsList should declare visual metadata");
    assert!(
        node_source[..end].contains("Some(chrome_tokens::LIQUID_GLASS_PANEL_RADIUS_PX)"),
        "ActionsList must use the shared Liquid Glass panel radius token"
    );
    assert!(
        !node_source[..end].contains("Some(0.0)"),
        "ActionsList must not satisfy guideline proof with a zero-radius placeholder"
    );
}
